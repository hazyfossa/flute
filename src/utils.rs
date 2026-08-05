pub mod error {
    use std::any::type_name;

    use hazymacros::trait_alias;

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

    hazymacros::vary!(v_impl);

    trait_alias!(pub AsResult: Into<Result<Self, eyre::Error>>);
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

    hazymacros::vary!(v_impl);
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
