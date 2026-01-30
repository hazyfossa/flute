use crate::trait_alias;
use snafu::{Snafu, Whatever};

#[derive(Debug, Snafu)]
pub struct AnyError(Whatever);

pub trait ErrorProvider<Error = AnyError> {
    type Error: Typed; // TODO: consider eros / exn
}
trait_alias!(pub trait Typed: std::error::Error + Send + 'static);
