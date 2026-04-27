pub mod compat;
pub mod tools;

pub mod core;
pub use core::*;

pub mod rpc;

mod macros {
    // TODO: generic bounds on alias are broken (local ambiguity of tt*)
    // this is fine since we do not use those in this crate
    #[macro_export]
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
