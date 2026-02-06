use std::convert::Infallible;

use flute::{
    error::ErrorProvider,
    transform::{TransformFraming, TransformRx, TransformTx},
};

// An example transform, which appends `len` zeroes to the end of payload
// This shouldn't be useful in any real case, only provided for demonstration
pub struct PaddingTransform {
    pad_len: usize,
}

impl ErrorProvider for PaddingTransform {
    type Error = Infallible;
}

// Note that the current framing model is quite limited
// as all buffers should be owned (this is a design decision)
//
// There is currently no proper way to handle sliced frames
// Such functionality depends on our (hopefully) upcoming IO model
// which will be used in flute V3
impl TransformFraming for PaddingTransform {
    type In = Vec<u8>;
    type Out = Vec<u8>;
}

impl TransformRx for PaddingTransform {
    fn decode(&mut self, mut data: Self::Out) -> Result<Self::In, Self::Error> {
        data.truncate(data.len() - self.pad_len);
        Ok(data)
    }
}

impl TransformTx for PaddingTransform {
    fn encode(&mut self, mut data: Self::In) -> Result<Self::Out, Self::Error> {
        data.append(&mut vec![0u8; self.pad_len]);
        Ok(data)
    }
}

fn main() {
    panic!("This is a library-style example")
}
