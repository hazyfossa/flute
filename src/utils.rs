mod macros {
    // TODO: generic bounds on alias are broken (local ambiguity of tt*)
    // this is fine since we do not use those in this crate
    #[macro_export(local_inner_macros)]
    macro_rules! trait_alias {
        (
            $(#[$($attr:meta)*])?
            $vis:vis trait $name:ident $(<$($generic:ident),*>)? : $($for:tt)*
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

    // TODO: consider relaxing bounds
    trait_alias!(pub trait Typed: std::error::Error + Send + Sync + 'static);

    pub trait ErrorProvider {
        type Error: Typed;

        // None means this provider is a shim
        // and should not be reported as error source
        fn name() -> Option<&'static str> {
            Some(type_name::<Self>())
        }
    }

    // BoxedErr is a type-erased error
    pub type BoxedError = Box<dyn Typed + 'static>;

    impl snafu::AsErrorSource for BoxedError {
        fn as_error_source(&self) -> &(dyn std::error::Error + 'static) {
            self.as_ref()
        }
    }

    impl<T: Typed> From<T> for BoxedError {
        fn from(value: T) -> Self {
            Box::new(value)
        }
    }

    // ErasedError is BoxedError with a provider context
    #[derive(Debug, Snafu)]
    pub struct ErasedError(Whatever);

    pub trait EraseResultExt<T, E>: Sized {
        fn erase_with_provider<P: ErrorProvider<Error = E>>(self) -> Result<T, ErasedError>;
    }

    impl<T, E> EraseResultExt<T, E> for Result<T, E>
    where
        E: Typed,
    {
        fn erase_with_provider<P: ErrorProvider<Error = E>>(self) -> Result<T, ErasedError> {
            use snafu::ResultExt;
            self.with_whatever_context(|_| match P::name() {
                // TODO: custom message
                Some(source) => format!("at: {source}"),
                None => String::new(), // TODO
            })
            .map_err(|e| ErasedError(e))
        }
    }

    mod variadics {
        macro_rules! v_impl {
            ($($t:ident)*) => {
                impl<$($t,)*> super::ErrorProvider for ( $($t,)* ) {
                    type Error = super::ErasedError;
                    fn name() -> Option<&'static str> { None }
                }
            };
        }

        v_impl!(A);
        v_impl!(A B);
        v_impl!(A B C);
        v_impl!(A B C D);
        v_impl!(A B C D E);
        v_impl!(A B C D E F);
        v_impl!(A B C D E F G);
        v_impl!(A B C D E F G H);
        v_impl!(A B C D E F G H I);
    }
}
