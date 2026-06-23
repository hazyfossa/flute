// Flow is an interface you may already know as "unframed transport".
// Currently, flows are a second-class interface in flute.
// This means you're way more likely to interact with channels instead.

use stable_deref_trait::StableDeref;
use std::ops::{Deref, DerefMut};

use crate::{error::ErrorProvider, trait_alias};

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
// see also BufTx::remaining
trait_alias!(pub trait Memory: Deref<Target = [byte]> + DerefMut + StableDeref);

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

// TODO: assertions about bounds, probably split const INIT for readability

/// The layout of Buf can be viewed as
/// [   data   |  init-junk  |     uninit     ]
/// [   data   |         tx-capacity          ]
/// [        init            |     uninit     ]
//
// TODO: consider replacing T here with something concrete.. ArrayVec?
//
// TODO: consider replacing init_end with const INIT: bool (do not support partial init)
// this essentially propagates the const from BufTx to Buf
// Pros: encodes performance characteristics in signature
// Cons: inconvenient generics
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

// TODO: Tx is confusing here: in flute it means Transmitter, here Transaction
pub struct BufTx<'a, T, const INIT: bool = false> {
    buf: &'a mut Buf<T>,

    // invariant: does not overflow `self.end()` or `usize`
    // when added to `buf.filled`
    written: usize,
}

impl<'a, T: Memory, const INIT: bool> BufTx<'a, T, INIT> {
    fn end(&self) -> usize {
        match INIT {
            true => self.buf.len(),
            false => self.buf.init_end,
        }
    }

    #[inline]
    pub fn written(&self) -> usize {
        self.written
    }

    /// Remaining is `self.substrate().len()`
    /// It is decreased upon `self.advance()`
    #[inline]
    pub fn remaining(&self) -> usize {
        // NOTE: we deliberately do not implement this as a call to `self.substrate().len()`
        // to allow `self.substrate()` to be unchecked.

        // TODO: here underflow is possible, meaning either
        // (1) buffer does not have enough space to handle the advance (happens with `INIT == true`)
        // (2) usize will overflow upon advance
        //        (1)                 (2)
        self.end() - self.buf.data_end - self.written
    }

    /// On failure, returns the overflow
    /// i.e. by how many bytes the capacity should increase
    /// for the advancement to be possible
    // TODO: consider propagating overflow of usize (which is underflow in capacity)
    // TODO: consider if unsafe advance_unchecked is even possible in a sane manner
    pub fn advance(&mut self, by: usize) -> Result<(), usize> {
        if by > self.remaining() {
            Err(self.remaining() - by)
        } else {
            // Safety: this will never overflow, guarded by capacity check above
            unsafe { self.written = self.written.unchecked_add(by) }
            Ok(())
        }
    }

    // The final signatures of the following functions vary on `const INIT`

    fn substrate_impl(&mut self) -> &mut [byte] {
        // Safety: overflow here is impossible per invariant of `self.written`
        unsafe {
            let start = self.buf.data_end.unchecked_add(self.written);
            let end = self.end();

            // TODO: although i have an idea why this range is always valid here,
            // it is difficult to reason about, since we allow buf.init_end < buf.data_end
            // is it sane to specify INIT == true implies buf.init_end >= buf.data_end???

            // TODO: do we lose perf here due to not using RangeFrom in the INIT == false case?
            self.buf.inner.get_unchecked_mut(start..end)
        }
    }

    // Safety: see (INIT == false)
    pub unsafe fn confirm_impl(self) -> usize {
        // Safety: this will never overflow by invariant of `written`
        unsafe { self.buf.data_end = self.buf.data_end.unchecked_add(self.written) };

        // Note: self.buf.init_end is not clamped here. This is intentional.

        self.written
    }
}

// On INIT == false, do not provide any additional guarantees
impl<'a, T: Memory> BufTx<'a, T, false> {
    /// Safety: while handling memory returned from here,
    /// you should never de-initialize bytes.
    ///
    /// The function itself is safe, but buffer layout
    /// requires [init | uninit] as continuous slices,
    /// so de-initializing arbitrary bytes makes `confirm` instant UB.
    pub fn substrate(&mut self) -> &mut [byte] {
        self.substrate_impl()
    }

    // Saefty: notes from `self.substrate()` apply
    pub fn as_ptr(&mut self) -> *mut [byte] {
        self.substrate() as _
    }

    /// This function confirms the transaction,
    /// returning the total amount of new bytes added to buffer
    ///
    /// Safety: by confirming, you guarantee that this transaction's memory
    /// (as returned by self.substrate()) now contains exactly `self.written()`
    /// initialized bytes (continuous, starting from beginning)
    pub unsafe fn confirm(self) -> usize {
        unsafe { self.confirm_impl() }
    }
}

impl<'a, T: Memory> BufTx<'a, T, true> {
    pub fn substrate(&mut self) -> &mut [u8] {
        // TODO: safety unclear, see `substrate_impl`
        unsafe { slice_assume_init_mut(self.substrate_impl()) }
    }

    pub fn confirm(self) -> usize {
        // Safety: since substrate of (INIT == true) transactions
        // is always fully initialized, this is always safe
        unsafe { self.confirm_impl() }
    }
}

// View

pub type MemoryView<T> = yoke::Yoke<&'static [u8], Buf<T>>;

impl<T: Memory> Buf<T> {
    pub fn data(self) -> MemoryView<T> {
        let filled = self.data_end;

        yoke::Yoke::attach_to_cart(self, |buf: &[byte]| {
            // Safety: self.filled will never overflow buf.len
            let slice = unsafe { buf.get_unchecked(..filled) };

            // Safety: [filled] is a subset of [init]
            unsafe { slice_assume_init(slice) }
        })
    }
}

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
}
