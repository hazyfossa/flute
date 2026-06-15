use crate::{error::ErrorProvider, state::Stateless};

pub trait DataFormat<Value>: Stateless + ErrorProvider {
    type Wire;

    fn encode(value: Value) -> Result<Self::Wire, Self::Error>;

    // TODO: decodes which borrow from wire (zero-copy)
    fn decode(wire: Self::Wire) -> Result<Value, Self::Error>;
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

    pub struct Postcard<const COBS: bool = false>;

    impl<const ANY: bool> Stateless for Postcard</* cobs: */ ANY> {}
    impl<const ANY: bool> ErrorProvider for Postcard</* cobs: */ ANY> {
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

    impl<V> DataFormat<V> for Postcard</* cobs: */ true>
    where
        V: serde::Serialize + serde::de::DeserializeOwned,
    {
        type Wire = Vec<u8>;

        fn encode(value: V) -> Result<Self::Wire, Self::Error> {
            ::postcard::to_stdvec_cobs(&value)
        }

        fn decode(mut wire: Self::Wire) -> Result<V, Self::Error> {
            ::postcard::from_bytes_cobs(&mut wire)
        }
    }
}
