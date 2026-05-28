mod macros {
    // TODO: generic bounds on alias are broken (local ambiguity of tt*)
    // this is fine since we do not use those in this crate
    #[macro_export(local_inner_macros)]
    macro_rules! trait_alias {
        (
            $(#[$($attr:meta)*])?
            $vis:vis trait $name:ident $(<$($generic:ident)+>)? : $($for:tt)*
            $(where $($generic_bound:tt)+)?
        ) => {
                $(#[$($attr)*])?
                $vis trait $name $(<$($generic,)+>)?: $($for)* {}
                impl<
                    Impl,
                    $($($generic,)+)?
                >
                $name $(<$($generic),+>)?
                for Impl where
                    Impl: $($for)*,
                    $($($generic_bound)+)?
                {}
            };
    }
}

pub mod error {
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
    trait_alias!(pub trait Typed: std::error::Error + Send + Sync + 'static);
    pub type BoxedErr = Box<dyn Typed + 'static>;

    impl snafu::AsErrorSource for BoxedErr {
        fn as_error_source(&self) -> &(dyn std::error::Error + 'static) {
            self.as_ref()
        }
    }

    impl<T: Typed> From<T> for BoxedErr {
        fn from(value: T) -> Self {
            Box::new(value)
        }
    }
}
