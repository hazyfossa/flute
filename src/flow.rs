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

// The layout of Buf can be viewed as
// [   data   |  init-junk  |     uninit     ]
// [   data   |         tx-capacity          ]
// [        init            |     uninit     ]
pub struct Buf<T> {
    inner: T,
    init: usize,
    filled: usize,
}
// TODO: consider replacing T here with something concrete.. ArrayVec?

// Buffer itself is memory
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
            init: 0,
            filled: 0,
        }
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
pub struct BufTx<'a, T> {
    buf: &'a mut Buf<T>,

    // Safety (invariant): does not overflow `buf.len` or `usize`
    // when added to `buf.filled`
    written: usize,
}

impl<'a, T: Memory> BufTx<'a, T> {
    /// Safety: while handling memory returned from here,
    /// you should never de-initialize bytes.
    ///
    /// The function itself is safe, but buffer layout
    /// requires [init | uninit] as continuous slices,
    /// so de-initializing arbitrary bytes makes `confirm` instant UB.
    pub fn substrate(&mut self) -> &mut [byte] {
        let start = self.buf.filled + self.written;

        // Safety: buf.filled does not overflow by invariant of Buf,
        // self.written does not overflow by invariant of BufTx
        unsafe { self.buf.inner.get_unchecked_mut(start..) }
    }

    /// Safety: notes from `self.remaining` apply.
    pub fn as_ptr(&mut self) -> *mut [byte] {
        self.substrate() as _
    }

    #[inline]
    pub fn written(&self) -> usize {
        self.written
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        let total = self.buf.len();
        total - self.buf.filled - self.written
    }

    /// On failure, returns the overflow
    /// i.e. by how many bytes the capacity should increase
    /// for the advancement to be possible
    // TODO: consider propagating overflow of usize (which is underflow in capacity)
    // TODO: consider if unsafe advance_unchecked is even possible in a sane manner
    pub fn advance(&mut self, by: usize) -> Result<(), usize> {
        if by > self.capacity() {
            Err(self.capacity() - by)
        } else {
            // Safety: this will never overflow, guarded by capacity check above
            unsafe { self.written = self.written.unchecked_add(by) }
            Ok(())
        }
    }

    /// This function confirms the transaction,
    /// returning the total amount of new bytes added to buffer
    ///
    /// Safety: by confirming, you guarantee that this transaction's memory
    /// (as returned by self.substrate()) now contains exactly `self.written()`
    /// initialized bytes (continuous, starting from beginning)
    pub unsafe fn confirm(self) -> usize {
        // Safety: this will never overflow by invariant of `written`
        unsafe { self.buf.filled = self.buf.filled.unchecked_add(self.written) };

        // TODO: consider not clamping here and instead doing arithmetic in request_init
        // Cons: non-trivial logic
        // Pros: moves the perf burden from common case (uninit) to API shim (init)
        self.buf.init = self.buf.init.max(self.buf.filled);
        self.written
    }
}

impl<T: Memory> Buf<T> {
    pub fn open_tx(&mut self) -> BufTx<'_, T> {
        BufTx {
            buf: self,
            written: 0,
        }
    }
}

// View

pub type MemoryView<T> = yoke::Yoke<&'static [u8], Buf<T>>;

impl<T: Memory> Buf<T> {
    pub fn data(self) -> MemoryView<T> {
        let filled = self.filled;

        yoke::Yoke::attach_to_cart(self, |buf: &[byte]| {
            // Safety: self.filled will never overflow buf.len
            let slice = unsafe { buf.get_unchecked(..filled) };

            // Safety: [filled] is a subset of [init]
            unsafe { slice_assume_init(slice) }
        })
    }
}

pub trait FlowRx: ErrorProvider {
    async fn recv<T: Memory>(&mut self, buf: &mut Buf<T>) -> Result<(), Self::Error>;
}

pub trait FlowTx: ErrorProvider {
    async fn send(&mut self, buf: &[u8]) -> Result<(), Self::Error>;
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
