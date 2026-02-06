use std::any::type_name;

use crate::trait_alias;
use snafu::{Snafu, Whatever};

#[derive(Debug, Snafu)]
pub struct AnyError(Whatever);

pub trait ErrorProvider<Error = AnyError> {
    type Error: Typed; // TODO: consider eros / exn
    fn name() -> &'static str {
        type_name::<Self>()
    }
}
trait_alias!(pub trait Typed: std::error::Error + Send + 'static);

impl snafu::AsErrorSource for Box<dyn Typed> {
    fn as_error_source(&self) -> &(dyn std::error::Error + 'static) {
        self.as_ref()
    }
}

impl<T: Typed> From<T> for Box<dyn Typed> {
    fn from(value: T) -> Self {
        Box::new(value)
    }
}
