mod macros {
    // TODO: generic bounds on alias are broken (local ambiguity of tt*)
    // this is fine since we do not use those in this crate
    #[macro_export(local_inner_macros)]
    macro_rules! trait_alias {
        (
            $(#[$($attr:meta)*])?
            $vis:vis trait $name:ident $(< $($life:lifetime)? $(,)? $($generic:ident),* >)? : $($for:tt)*
            $(where $($generic_bound:tt)+)?
        ) => {
                $(#[$($attr)*])?
                $vis trait $name $(< $($life,)* $($generic,)*>)?: $($for)* {}
                impl<
                    $($($life,)?)?
                    Impl,
                    $($($generic,)*)?
                >
                $name $(< $($life,)? $($generic),* >)?
                for Impl where
                    Impl: $($for)*,
                    $($($generic_bound)+)?
                {}
            };
    }
}

pub mod error {
    use std::any::type_name;

    pub trait ErrorProvider {
        type Error: Into<eyre::Error>;

        // None means this provider is a shim
        // and should not be reported as error source
        fn name() -> Option<&'static str> {
            Some(type_name::<Self>())
        }
    }

    pub trait EraseResultExt<T, E>: Sized {
        fn erase_with_provider<P: ErrorProvider<Error = E>>(self) -> eyre::Result<T>;
    }

    impl<T, E> EraseResultExt<T, E> for Result<T, E>
    where
        E: Into<eyre::Error>,
    {
        fn erase_with_provider<P: ErrorProvider<Error = E>>(self) -> eyre::Result<T> {
            let ret = self.map_err(|e| e.into());
            use eyre::WrapErr;

            match P::name() {
                Some(source) => ret.wrap_err_with(|| format!("at: {}", source)),
                None => ret,
            }
        }
    }

    mod variadics {
        use super::*;
        macro_rules! v_impl {
            ($($t:ident)*) => {
                impl<$($t,)*> ErrorProvider for ( $($t,)* ) {
                    type Error = eyre::Error;
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

pub mod state {

    // TODO: ponder
    pub trait Stateless: Sized {
        fn stateless() -> Self {
            const { assert!(core::mem::size_of::<Self>() == 0) }

            // SAFETY: The `assert!` above guarantees `T` is zero-sized.
            unsafe { core::mem::zeroed() }
        }
    }

    mod variadics {
        use super::*;
        macro_rules! v_impl {
            ($($t:ident)*) => {
                impl<$($t,)*> Stateless for ( $($t,)* )
                where $($t: Stateless,)* {
                    fn stateless() -> Self {
                        ($($t::stateless(),)*)
                    }
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

pub mod branches {
    #[inline(always)]
    #[cold]
    fn mark_uncommon_case() {}

    pub fn likely(b: bool) -> bool {
        if !b {
            mark_uncommon_case();
        }
        b
    }

    pub fn unlikely(b: bool) -> bool {
        if b {
            mark_uncommon_case();
        }
        b
    }
}
