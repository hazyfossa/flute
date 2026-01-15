use eyre::{Result, eyre};
use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;

pub trait DataFormat {
    type Repr;

    fn encode<T: Serialize>(&mut self, value: T) -> Result<Self::Repr>;
    fn decode<T: DeserializeOwned>(&mut self, data: Self::Repr) -> Result<T>;
}

pub struct CodecError<C: Codec + ?Sized>(C::Error);

impl<C: Codec<Error = T> + ?Sized, T: std::error::Error + Send + Sync + 'static> From<T>
    for CodecError<C>
{
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<C: Codec + ?Sized> From<CodecError<C>> for eyre::Error {
    fn from(value: CodecError<C>) -> Self {
        eyre!(value.0).wrap_err(format!("{} codec error", C::NAME))
    }
}

pub trait Codec {
    const NAME: &str;

    type In;
    type Out;
    type Error: std::error::Error + Send + Sync + 'static;

    fn encode(&mut self, data: Self::In) -> Result<Self::Out, CodecError<Self>>;
    fn decode(&mut self, data: Self::Out) -> Result<Self::In, CodecError<Self>>;
}

pub struct WithCodec<Ch, Co> {
    channel: Ch,
    codec: Co,
}

impl<Ch, Co, In, Out> Channel for WithCodec<Ch, Co>
where
    Ch: Channel<Wire = Out>,
    Co: Codec<In = In, Out = Out>,
{
    type Wire = In;

    async fn recv(&mut self) -> Result<Self::Wire, ChannelError> {
        let data = self.channel.recv().await?;
        Ok(self.codec.decode(data)?)
    }

    async fn send(&mut self, data: Self::Wire) -> Result<(), ChannelError> {
        let data = self.codec.encode(data)?;
        Ok(self.channel.send(data).await?)
    }
}

impl<C: Codec> From<CodecError<C>> for ChannelError {
    fn from(value: CodecError<C>) -> Self {
        Self::Transport(value.into())
    }
}

#[derive(Debug, Error)]
pub enum ChannelError {
    #[error("channel closed")]
    Closed,
    #[error(transparent)]
    Transport(#[from] eyre::Error),
}

#[allow(async_fn_in_trait)]
pub trait Channel {
    type Wire;

    async fn recv(&mut self) -> Result<Self::Wire, ChannelError>;
    async fn send(&mut self, data: Self::Wire) -> Result<(), ChannelError>;

    fn with_codec<C: Codec>(self, codec: C) -> WithCodec<Self, C>
    where
        Self: Sized,
    {
        WithCodec {
            channel: self,
            codec,
        }
    }
}
