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

    #[macro_export(local_inner_macros)]
    macro_rules! vary {
        ($v_impl:tt) => {
            $v_impl!(T1);
            $v_impl!(T1 T2);
            $v_impl!(T1 T2 T3);
            $v_impl!(T1 T2 T3 T4);
            $v_impl!(T1 T2 T3 T4 T5);
            $v_impl!(T1 T2 T3 T4 T5 T6);
            $v_impl!(T1 T2 T3 T4 T5 T6 T7);
            $v_impl!(T1 T2 T3 T4 T5 T6 T7 T8);
            $v_impl!(T1 T2 T3 T4 T5 T6 T7 T8 T9);
            $v_impl!(T1 T2 T3 T4 T5 T6 T7 T8 T9 T10);
        };
    }
}

pub mod error {
    use std::any::type_name;

    use crate::trait_alias;

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

    macro_rules! v_impl {
        ($($t:ident)*) => {
            impl<$($t,)*> ErrorProvider for ( $($t,)* ) {
                type Error = eyre::Error;
                fn name() -> Option<&'static str> { None }
            }
        };
    }

    crate::vary!(v_impl);

    trait_alias!(pub trait AsResult: Into<Result<Self, eyre::Error>>);
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

    crate::vary!(v_impl);
}

pub mod branches {
    #[inline(always)]
    #[cold]
    fn mark_uncommon_case() {}

    // pub fn likely(b: bool) -> bool {
    //     if !b {
    //         mark_uncommon_case();
    //     }
    //     b
    // }

    pub fn unlikely(b: bool) -> bool {
        if b {
            mark_uncommon_case();
        }
        b
    }
}

pub mod delayed_mut {
    use std::{marker::PhantomData, ptr};

    pub struct DelayedMut<'a, T> {
        raw: *mut T,
        _life: PhantomData<&'a mut ()>,
    }

    impl<'a, T> From<&'a mut T> for DelayedMut<'a, T> {
        fn from(value: &'a mut T) -> Self {
            Self {
                raw: ptr::from_mut(value),
                _life: PhantomData,
            }
        }
    }

    impl<'a, T> DelayedMut<'a, T> {
        /// Safety: by calling this method, you guarantee this
        /// pointer is the only pointer to T
        pub unsafe fn guarantee_single_instance(self) -> &'a mut T {
            unsafe { &mut *self.raw }
        }
    }
}
