// Flow is an interface you may already know as "unframed transport".
// Currently, flows are a second-class interface in flute.
// This means you're way more likely to interact with channels instead.

use stable_deref_trait::StableDeref;
use std::{
    hint::black_box,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};
use thiserror::Error;
use yoke::{Yoke, Yokeable};

use crate::{error::ErrorProvider, trait_alias, utils::branches::unlikely};

/// Flute's byte, unlike rust's "u8", has 257 possible states.
///
/// The extra state is "uninit". This tells the compiler we
/// do not care about the actual value, since it's only purpose
/// is to be overwritten.
///
/// You should almost always prefer u8 instead,
/// unless allocating memory for a Buf.
#[allow(non_camel_case_types)]
type byte = std::mem::MaybeUninit<u8>;

// TODO: the DerefMut here allows de-initializing arbitrary bytes
// which is is a non-trivial footgun. This means that either:
// 1. Buf cannot implement Memory (current) -> does this even matter?
// 2. Buf cannot safely slice_assume_init on Memory -> we need separate Memory / MemoryMut
trait_alias!(pub trait Memory: Deref<Target = [byte]> + DerefMut + StableDeref + 'static);

pub fn alloc_heap(size: usize) -> Box<[byte]> {
    Box::new_uninit_slice(size)
}

pub fn alloc_stack<const S: usize>() -> [byte; S] {
    [byte::uninit(); S]
}

/// # Safety
///
/// The caller must ensure that `slice` is fully initialized.
unsafe fn slice_assume_init(slice: &[byte]) -> &[u8] {
    // SAFETY: `MaybeUninit<u8>` has the same memory layout as u8, and the caller
    // promises that `slice` is fully initialized.
    unsafe { &*(slice as *const [byte] as *const [u8]) }
}

/// # Safety
///
/// The caller must ensure that `slice` is fully initialized.
unsafe fn slice_assume_init_mut(slice: &mut [byte]) -> &mut [u8] {
    // SAFETY: `MaybeUninit<u8>` has the same memory layout as `u8`, and the caller
    // promises that `slice` is fully initialized.
    unsafe { &mut *(slice as *mut [byte] as *mut [u8]) }
}

#[derive(Debug, Error)]
pub enum RollError {
    #[error("usize overflow")]
    UsizeOverflow,
    #[error("buffer overflow")]
    BufferOverflow { by: usize },
}

// Roll is a cursor which only goes forwards,
// making previously accessed parts of a slice inaccessible
#[derive(Yokeable)]
struct Roll<T> {
    inner: T,
    pub pos: usize,
}

impl<T> Roll<T> {
    fn new(inner: T) -> Self {
        Self { inner, pos: 0 }
    }

    fn into_inner(self) -> T {
        self.inner
    }
}

impl<T, U> Roll<T>
where
    T: Deref<Target = [U]>,
{
    #[inline]
    fn passed(&self) -> usize {
        self.pos
    }

    fn remaining(&self) -> usize {
        self.inner.len() - self.passed()
    }

    fn advance(&mut self, by: usize) -> Result<(), RollError> {
        let added = by;

        if unlikely(added > self.remaining()) {
            return Err(RollError::BufferOverflow {
                by: added - self.remaining(),
            });
        }

        self.pos = self
            .pos
            .checked_add(added)
            .ok_or(RollError::UsizeOverflow)?;

        Ok(())
    }

    /// Safety: the caller should guaratee that
    /// 1. self.pos + by does not overflow usize
    /// 2. `by` does not overflow `self.len()`
    #[inline]
    unsafe fn advance_unchecked(&mut self, by: usize) {
        unsafe { self.pos = self.pos.unchecked_add(by) }
    }

    #[inline]
    fn get_ref(&self) -> &[U] {
        // Safety: self.pos is <= self.inner.len() by invariant
        unsafe { self.inner.get_unchecked(self.pos..) }
    }
}

impl<T, U> Roll<T>
where
    T: DerefMut<Target = [U]>,
{
    #[inline]
    fn get_mut(&mut self) -> &mut [U] {
        // Safety: self.pos is <= self.inner.len() by invariant
        unsafe { self.inner.get_unchecked_mut(self.pos..) }
    }
}

// TODO: assertions about bounds, probably split const INIT for readability

/// The layout of Buf can be viewed as
/// [   data   |  init-substrate  |     uninit     ]
/// [   data   |              substrate            ]
/// [        init                 |     uninit     ]
//
// TODO: consider replacing T here with something concrete.. ArrayVec?
pub struct Buf<T> {
    inner: T,
    data_end: usize,

    // note: init_end <= data_end means this buffer does not have
    // excess init bytes (all of them are relevant data)
    init_end: usize,
}

// Buffer itself is memory
// TODO: this is problematic for IDE hints, but is required by yoke
impl<T: Memory> Deref for Buf<T> {
    type Target = [byte];

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

// Safety: this just propagates the safety properties of `inner` memory
unsafe impl<T: Memory> StableDeref for Buf<T> {}

impl<T: Memory> Buf<T> {
    pub fn new(memory: T) -> Self {
        Self {
            inner: memory,
            init_end: 0,
            data_end: 0,
        }
    }

    pub fn open_tx(&mut self) -> BufTx<'_, T> {
        BufTx {
            buf: self,
            written: 0,
        }
    }

    pub fn open_tx_partial_init(&mut self, len: usize) -> BufTx<'_, T, true> {
        let start = self.init_end.max(self.data_end);
        let end = start + len; // TODO: this can overflow

        unsafe {
            self.inner[start..end]
                .as_mut_ptr()
                .write_bytes(0, end - start);
        }

        BufTx {
            buf: self,
            written: 0,
        }
    }

    pub fn open_tx_init(&mut self) -> BufTx<'_, T, true> {
        self.open_tx_partial_init(self.len())
    }
}

// TODO: this should be replaced by scatter/gather (vectored io)
impl<T: Extend<byte>> Extend<byte> for Buf<T> {
    fn extend<U: IntoIterator<Item = byte>>(&mut self, iter: U) {
        self.inner.extend(iter);
    }
}

// Transaction

// NOTE: we could get rid of yoke here and simplify API. Some approaches
// 1. unsafe *mut ptr of Buf: works for unsafe confirm, not so much for INIT Tx
// 2. returning Roll on open_tx, confirm becomes a method on buf -> much more complex confirm safety
pub type BorrowedRoll<U> = Roll<&'static mut [U]>;

// TODO: Tx is confusing here: in flute it means Transmitter, here Transaction
// TODO: this currently prohibits async write tx as a limitation of yoke
pub struct WriteTx<'a, T, U: 'static> {
    inner: Yoke<BorrowedRoll<U>, &'a mut Buf<T>>,
}

impl<'a, T: Memory, U> WriteTx<'a, T, U> {
    pub fn written(&self) -> usize {
        self.inner.get().passed()
    }

    pub fn peek(&self) -> &[U] {
        self.inner.get().get_ref()
    }

    pub fn with_roll<F, Out>(&mut self, f: F) -> Out
    where
        F: for<'b> FnOnce(&'b mut BorrowedRoll<U>) -> Out,
        Out: 'static,
        F: 'static,
    {
        self.inner.with_mut_return(f)
    }

    /// Performs a transactional operation
    /// Accepts a function F(memory) -> Result<bytes_written>
    ///
    // TODO: express the following as a newtype
    /// If you're working with uninitialized bytes, remember to never de-initialize bytes
    /// while this function will allow you to write something like
    /// ```
    /// memory[arbitrary_index] = byte::uninit()
    /// ```
    /// doing so is invalid and will make .confirm() instant UB
    pub fn act<F, E>(&mut self, f: F) -> Result<(), E>
    where
        F: FnOnce(&mut [U]) -> Result<usize, E>,
        F: 'static,
        E: From<RollError> + 'static,
    {
        self.with_roll(|roll| {
            let ret = f(roll.get_mut())?;
            roll.advance(ret)?;
            Ok(())
        })
    }

    unsafe fn confirm_impl(self) -> usize {
        let added = self.inner.get().passed();
        let buf = self.inner.into_backing_cart();
        // TODO: this can overflow usize, but not buf.len()
        buf.data_end += added;
        added
    }
}

impl<'a, T: Memory> WriteTx<'a, T, byte> {
    /// Safety: by confirming, you guarantee that this transaction's memory
    /// now contains `self.written()` initialized bytes (continuous, starting from beginning)
    pub unsafe fn confirm(self) -> usize {
        unsafe { self.confirm_impl() }
    }
}

impl<'a, T: Memory> WriteTx<'a, T, u8> {
    pub fn confirm(self) -> usize {
        // Safety: since this Tx is init (u8), confirm is always safe
        // Worst case, on caller error, data will contain some initialized junk padding (zeroes)
        unsafe { self.confirm_impl() }
    }
}

// Reading data

pub type MemoryView<T> = yoke::Yoke<&'static [u8], Buf<T>>;

// impl<T: Memory> Buf<T> {
//     pub fn view_data(self) -> MemoryView<T> {
//         let data_end = self.data_end;

//         yoke::Yoke::attach_to_cart(self, |mem: &[byte]| {
//             Buf {
//                 inner: mem,
//                 data_end,
//                 init_end,
//             }
//             .data()
//         })
//     }
// }

// TODO: the usize rets are redundant, we store that info in the buffer
// counterargument: EOF detection

pub trait FlowRx: ErrorProvider {
    async fn recv<T: Memory>(&mut self, buf: &mut Buf<T>) -> Result<usize, Self::Error>;
}

pub trait FlowTx: ErrorProvider {
    async fn send<T: Memory>(&mut self, buf: &mut Buf<T>) -> Result<usize, Self::Error>;
}

mod frame {
    use super::*;
    use crate::{Rx, Tx};

    pub struct EvenChunks<F> {
        flow: F,
        size: usize,
        buf: Vec<u8>, // TODO: alloc
    }
}

#[cfg(feature = "bytes")]
mod bytes {
    use ::bytes::{Buf, BufMut, buf::UninitSlice};

    use super::*;

    unsafe impl<T: DerefMut> BufMut for Roll<T> {
        unsafe fn advance_mut(&mut self, cnt: usize) {
            self.advance(cnt).unwrap();
        }

        fn remaining_mut(&self) -> usize {
            self.remaining()
        }
    }
}

#[cfg(feature = "tokio")]
pub mod tokio {
    use super::{Buf, FlowRx, FlowTx, Memory};
    use crate::error::ErrorProvider;
    use tokio::io::ReadBuf as TokioBuf;

    pub struct Adapt<T>(T);

    impl<T> ErrorProvider for Adapt<T> {
        type Error = tokio::io::Error;
    }

    impl<T: tokio::io::AsyncRead + Unpin> FlowRx for Adapt<T> {
        async fn recv<M: Memory>(&mut self, buf: &mut Buf<M>) -> Result<usize, Self::Error> {
            let mut buf = buf.open_tx();
            let mut buf = TokioBuf::uninit(buf.substrate());

            use tokio::io::AsyncReadExt;
            let ret = self.0.read_buf(&mut buf).await?;

            Ok(ret)
        }
    }

    // impl<T: tokio::io::AsyncWrite + Unpin> FlowTx for Adapt<T> {
    //     async fn send<M: Memory>(&mut self, buf: &mut Buf<M>) -> Result<usize, Self::Error> {
    //         let mut buf = buf.open_tx();
    //         let mut buf = TokioBuf::uninit(buf.substrate());

    //         use tokio::io::AsyncWriteExt;
    //         let ret = self.0.write_buf(&mut buf).await?;

    //         Ok(ret)
    //     }
    // }
}
