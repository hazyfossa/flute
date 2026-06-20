// Flow is an interface you may already know as "unframed transport".
// Currently, flows are a second-class interface in flute.
// This means you're way more likely to interact with channels instead.

use std::io::Cursor;

use crate::error::ErrorProvider;

pub struct FlowReceived<T, E> {
    buffer: T,
    ret: Result<usize, E>,
}

impl<T, E> FlowReceived<T, E> {
    pub fn ref_buffer(&self) -> &T {
        &self.buffer
    }

    // Disregards data about received bytes.
    // You should prefer `catch_*` or `view_*` instead.
    pub fn melt_into_buffer(self) -> T {
        self.buffer
    }
}

impl<T: AsRef<[u8]>, E> FlowReceived<T, E> {
    pub fn new(buffer: T, ret: Result<usize, E>) -> Self {
        // TODO: this is suboptimal for compiler elision of bound checks
        if let Ok(byte_count) = ret {
            if buffer.as_ref().len() < byte_count {
                panic!("Out of bounds: byte_count > buffer.len()")
            }
        }

        Self { buffer, ret }
    }

    /// Safety: you should be sure `byte_count` is within bounds of `buffer`.
    /// In other words, buffer[byte_cound] should be valid and safe.
    pub unsafe fn new_unchecked(buffer: T, ret: Result<usize, E>) -> Self {
        Self { buffer, ret }
    }
}

impl<T: AsMut<[u8]>, E> FlowReceived<T, E> {
    /// Returns (read, remaining)
    /// where read is the received data
    /// and remaining is whatever was previously in buffer
    // TODO: &mut E is unintuitive
    pub fn view_split(&mut self) -> Result<(&mut [u8], &mut [u8]), &mut E> {
        let byte_count = self.ret.as_mut()?;

        let all = self.buffer.as_mut();

        // Safety: `self.byte_cound` is within bounds of `self.buffer` by construction
        let split = unsafe { all.split_at_mut_unchecked(*byte_count) };

        Ok(split)
    }
}

impl<T, E> FlowReceived<Cursor<T>, E> {
    pub fn catch_as_cursor(self) -> Result<Cursor<T>, E> {
        let byte_count = self.ret?;
        let mut cursor = self.buffer;

        let previous = cursor.position();
        cursor.set_position(previous + byte_count as u64);

        Ok(cursor)
    }
}

pub trait FlowRx: ErrorProvider {
    // TODO: make T support MaybeUninit
    // (flows can assume `ret` bytes are init after syscall where `ret is syscall's byte_count return`)
    async fn recv<T: AsMut<[u8]>>(buf: T) -> FlowReceived<T, Self::Error>;
}

pub struct FlowSent<T, E> {
    pub buffer: T,
    pub result: Result<(), E>,
}

pub trait FlowTx: ErrorProvider {
    async fn send<T: AsRef<[u8]>>(buf: T) -> FlowSent<T, Self::Error>;
}

mod frame {
    use super::*;
    use crate::{Rx, Tx};

    pub struct EvenChunks<F> {
        flow: F,
        size: usize,
        buf: Vec<u8>, // TODO: alloc
    }

    impl<F: FlowRx> Rx for EvenChunks<F> {
        async fn recv(&mut self) -> Result<Self::Out, crate::ChannelError> {}
    }
}
