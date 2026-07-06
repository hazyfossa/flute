use std::marker::PhantomData;

use crate::{error::ErrorProvider, state::Stateless, transform::Transform};

// DataFormat is an alias for a common kind of Transform
pub trait DataFormat<Value>: Stateless + ErrorProvider {
    type Wire;

    fn encode(value: Value) -> Result<Self::Wire, Self::Error>;

    // TODO: decodes which borrow from wire (zero-copy)
    fn decode(wire: Self::Wire) -> Result<Value, Self::Error>;
}

pub struct DataTransform<T, Value> {
    phantom: PhantomData<(T, Value)>,
}

impl<T: DataFormat<Any>, Any> Stateless for DataTransform<T, Any> {}

impl<T: DataFormat<Any>, Any> ErrorProvider for DataTransform<T, Any> {
    type Error = T::Error;
}

impl<T: DataFormat<Value>, Value> Transform for DataTransform<T, Value> {
    type Before = Value;
    type After = T::Wire;

    fn encode(&mut self, before: Self::Before) -> Result<Self::After, Self::Error> {
        T::encode(before)
    }

    fn decode(&mut self, after: Self::After) -> Result<Self::Before, Self::Error> {
        T::decode(after)
    }
}

#[cfg(feature = "data-json")]
pub mod json {
    use super::*;

    pub struct Json;

    impl Stateless for Json {}
    impl ErrorProvider for Json {
        type Error = serde_json::Error;
    }

    impl<V> DataFormat<V> for Json
    where
        V: serde::Serialize + serde::de::DeserializeOwned,
    {
        type Wire = String;

        fn encode(value: V) -> Result<Self::Wire, Self::Error> {
            serde_json::to_string(&value)
        }

        fn decode(wire: Self::Wire) -> Result<V, Self::Error> {
            serde_json::from_str(&wire)
        }
    }
}

#[cfg(feature = "data-postcard")]
pub mod postcard {
    use super::*;

    pub struct Postcard;

    impl Stateless for Postcard {}
    impl ErrorProvider for Postcard {
        type Error = ::postcard::Error;
    }

    impl<V> DataFormat<V> for Postcard
    where
        V: serde::Serialize + serde::de::DeserializeOwned,
    {
        type Wire = Vec<u8>;

        fn encode(value: V) -> Result<Self::Wire, Self::Error> {
            ::postcard::to_stdvec(&value)
        }

        fn decode(wire: Self::Wire) -> Result<V, Self::Error> {
            ::postcard::from_bytes(&wire)
        }
    }
}
