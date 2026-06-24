// Flow is an interface you may already know as "unframed transport".
// Currently, flows are a second-class interface in flute.
// This means you're way more likely to interact with channels instead.

use stable_deref_trait::StableDeref;
use std::ops::{Deref, DerefMut};
use thiserror::Error;
use yoke::Yokeable;

use crate::{
    error::ErrorProvider,
    trait_alias,
    utils::{branches::unlikely, delayed_mut::DelayedMut},
};

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
pub struct Roll<T> {
    inner: T,
    pub pos: usize,
}

impl<T> Roll<T> {
    pub fn new(inner: T) -> Self {
        Self { inner, pos: 0 }
    }

    pub fn into_inner(self) -> T {
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
    pub unsafe fn advance_unchecked(&mut self, by: usize) {
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

/// The layout of Buf can be viewed as
/// [   data   |  init-substrate  |     uninit     ]
/// [   data   |              substrate            ]
/// [        init                 |     uninit     ]
pub struct Buf<T> {
    inner: T,
    data_end: usize,

    // invariant: if init_end >= data_end,
    // buf[data_end..init_end] is initialized
    //
    // note: init_end < data_end is possible
    // this means buffer does not have excess init bytes
    // (all of them are relevant data)
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

    pub fn data(&self) -> &[u8] {
        let data_section = &self.inner[..self.data_end];

        // Safety: this cas is safe by invariant of Buf
        // ([data] is a subset of [init])
        unsafe { slice_assume_init(data_section) }
    }

    pub fn data_mut(&mut self) -> &mut [u8] {
        let data_section = &mut self.inner[..self.data_end];

        // Safety: this cas is safe by invariant of Buf
        // ([data] is a subset of [init])
        unsafe { slice_assume_init_mut(data_section) }
    }

    pub fn open_rx(&self) -> Roll<&[u8]> {
        Roll::new(self.data())
    }

    pub fn open_tx(&mut self) -> WriteTx<'_, byte> {
        WriteTx::new(
            &mut self.inner[self.data_end..],
            (&mut self.data_end).into(),
        )
    }

    pub fn open_tx_partial_init(&mut self, len: usize) -> WriteTx<'_, u8> {
        let excess_init = self.init_end.saturating_sub(self.data_end);

        if excess_init == 0 {
            // clamp in case the subtraction saturated
            // so init_end is >= self.data_end
            self.init_end = self.data_end
        }

        let (write_start, write_len) = (self.init_end + excess_init, len - excess_init);

        unsafe {
            let _ = &mut self.inner[write_start..write_start + write_len]
                .as_mut_ptr()
                .write_bytes(0, len);
        };

        self.init_end += write_len;

        let init_section = &mut self.inner[self.data_end..self.init_end];

        // Safety: cast valid by invariant of init_end
        let init_section = unsafe { slice_assume_init_mut(init_section) };

        WriteTx::new(init_section, (&mut self.data_end).into())
    }

    pub fn open_tx_init(&mut self) -> WriteTx<'_, u8> {
        // TODO: we can save 1-2 cpu instructions here with a specialized impl
        self.open_tx_partial_init(self.inner.len())
    }
}

// TODO: this should be replaced by scatter/gather (vectored io)
impl<T: Extend<byte>> Extend<byte> for Buf<T> {
    fn extend<U: IntoIterator<Item = byte>>(&mut self, iter: U) {
        self.inner.extend(iter);
    }
}

// TODO: Tx is confusing here: in flute it means Transmitter, here Transaction
pub struct WriteTx<'a, T> {
    inner: Roll<&'a mut [T]>,
    buf_data_end: DelayedMut<'a, usize>,
}

impl<'a, T> Deref for WriteTx<'a, T> {
    type Target = Roll<&'a mut [T]>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a, T> DerefMut for WriteTx<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<'a, T> WriteTx<'a, T> {
    fn new(inner: &'a mut [T], data_end: DelayedMut<'a, usize>) -> Self {
        Self {
            inner: Roll::new(inner),
            buf_data_end: data_end,
        }
    }

    pub fn as_roll(&mut self) -> &mut Roll<&'a mut [T]> {
        &mut self.inner
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
        F: FnOnce(&mut [T]) -> Result<usize, E>,
        E: From<RollError> + 'static,
    {
        let ret = f(self.get_mut())?;
        self.advance(ret)?;
        Ok(())
    }

    /// Safety: by confirming, you guarantee that this transaction's memory
    /// now contains `self.written()` initialized bytes (continuous, starting from beginning)
    unsafe fn confirm_impl(self) -> usize {
        let WriteTx {
            inner,
            buf_data_end,
        } = self;
        let added = inner.passed();
        drop(inner);

        // Safety: self.inner is dropped, so this is the only mutable pointer to buf now
        unsafe {
            *buf_data_end.guarantee_single_instance() += added;
        }
        added
    }
}

impl<'a> WriteTx<'a, byte> {
    /// Safety: by confirming, you guarantee that this transaction's memory
    /// now contains `self.written()` initialized bytes (continuous, starting from beginning)
    #[must_use]
    pub unsafe fn confirm(self) -> usize {
        unsafe { self.confirm_impl() }
    }
}

impl<'a> WriteTx<'a, u8> {
    #[must_use]
    pub fn confirm(self) -> usize {
        // Safety: since this Tx is init (u8), confirm is always safe
        // Worst case, on caller error, data will contain some initialized junk padding (zeroes)
        unsafe { self.confirm_impl() }
    }
}

// Reading data

pub type MemoryView<T> = yoke::Yoke<&'static [u8], Buf<T>>;

impl<T: Memory> Buf<T> {
    pub fn view_data(self) -> MemoryView<T> {
        let data_end = self.data_end;

        yoke::Yoke::attach_to_cart(self, |mem: &[byte]| {
            let data_section = &mem[..data_end];

            // Safety: this cas is safe by invariant of Buf
            // ([data] is a subset of [init])
            unsafe { slice_assume_init(data_section) }
        })
    }
}

pub trait FlowRx: ErrorProvider {
    async fn recv<T: Memory>(&mut self, buf: &mut Buf<T>) -> Result<usize, Self::Error>;
}

pub trait FlowTx: ErrorProvider {
    async fn send<T: Memory>(&mut self, buf: &mut Buf<T>) -> Result<usize, Self::Error>;
}

mod frame {
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

    impl<T: Deref<Target = [u8]>> Buf for Roll<T> {
        fn advance(&mut self, cnt: usize) {
            self.advance(cnt).unwrap();
        }

        fn chunk(&self) -> &[u8] {
            self.get_ref()
        }

        fn remaining(&self) -> usize {
            self.remaining()
        }
    }

    // TODO: impls for init Rolls (u8) will require Roll to replace Deref with AsRef
    // (losing us owned Rolls)
    unsafe impl<T: DerefMut<Target = [byte]>> BufMut for Roll<T> {
        unsafe fn advance_mut(&mut self, cnt: usize) {
            self.advance(cnt).unwrap();
        }

        fn remaining_mut(&self) -> usize {
            self.remaining()
        }

        fn chunk_mut(&mut self) -> &mut UninitSlice {
            UninitSlice::uninit(self.get_mut())
        }
    }
}

#[cfg(feature = "tokio")]
pub mod tokio {
    use super::{Buf, FlowRx, Memory};
    use crate::{error::ErrorProvider, flow::FlowTx};

    pub struct Adapt<T>(T);

    impl<T> ErrorProvider for Adapt<T> {
        type Error = tokio::io::Error;
    }

    impl<T: tokio::io::AsyncRead + Unpin> FlowRx for Adapt<T> {
        async fn recv<M: Memory>(&mut self, buf: &mut Buf<M>) -> Result<usize, Self::Error> {
            let mut tx = buf.open_tx();

            use tokio::io::AsyncReadExt;
            self.0.read_buf(tx.as_roll()).await?;

            // Safety: tokio's guarantees about buffer layout align with ours
            // For more info, see bytes::BufMut::advance (which is unsafe)
            Ok(unsafe { tx.confirm() })
        }
    }

    impl<T: tokio::io::AsyncWrite + Unpin> FlowTx for Adapt<T> {
        async fn send<M: Memory>(&mut self, buf: &mut Buf<M>) -> Result<usize, Self::Error> {
            use tokio::io::AsyncWriteExt;
            let ret = self.0.write_buf(&mut buf.open_rx()).await?;

            Ok(ret)
        }
    }
}
