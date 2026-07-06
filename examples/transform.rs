use std::convert::Infallible;

use flute::{error::ErrorProvider, state::Stateless, transform::Transform};

// An example transform, which appends `len` zeroes to the end of payload
// This shouldn't be useful in any real case, only provided for demonstration
pub struct PaddingTransform<const PAD_LEN: usize>;

impl<const ANY: usize> Stateless for PaddingTransform<ANY> {}
impl<const ANY: usize> ErrorProvider for PaddingTransform<ANY> {
    type Error = Infallible;
}

impl<const PAD_LEN: usize> Transform for PaddingTransform<PAD_LEN> {
    type Before = Vec<u8>;
    type After = Vec<u8>;

    fn decode(&mut self, mut data: Self::After) -> Result<Self::Before, Self::Error> {
        data.truncate(data.len() - PAD_LEN);
        Ok(data)
    }

    fn encode(&mut self, mut data: Self::Before) -> Result<Self::After, Self::Error> {
        data.append(&mut vec![0u8; PAD_LEN]);
        Ok(data)
    }
}

// You can create very complex transforms using transform_alias
flute::transform_alias!(pub PadTwice = PaddingTransform<10> => PaddingTransform<20>);

// Aliases can be configurable. Specify the configuration inside [brackets].
// The configuration syntax is exactly equal to rust generics.
flute::transform_alias!(
    pub PadTwiceConfigurable
    [const A: usize, const B: usize,]
    = PaddingTransform<A> => PaddingTransform<B>
);

// TODO: stateful transform aliases

fn main() {
    panic!("This is a library-style example")
}
