use std::convert::Infallible;

use flute::{error::ErrorProvider, transform::Transform};

// An example transform, which appends `len` zeroes to the end of payload
// This shouldn't be useful in any real case, only provided for demonstration
pub struct PaddingTransform {
    pad_len: usize,
}

impl ErrorProvider for PaddingTransform {
    type Error = Infallible;
}

impl Transform for PaddingTransform {
    type Before = Vec<u8>;
    type After = Vec<u8>;

    fn decode(&mut self, mut data: Self::After) -> Result<Self::Before, Self::Error> {
        data.truncate(data.len() - self.pad_len);
        Ok(data)
    }

    fn encode(&mut self, mut data: Self::Before) -> Result<Self::After, Self::Error> {
        data.append(&mut vec![0u8; self.pad_len]);
        Ok(data)
    }
}

fn main() {
    panic!("This is a library-style example")
}
