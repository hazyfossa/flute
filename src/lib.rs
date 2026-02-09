pub mod compat;
pub mod tools;

pub mod core;
pub use core::*;

pub mod rpc;

mod macros {
    #[macro_export]
    macro_rules! trait_alias {
        ($vis:vis trait $name:ident : $($for:tt)*) => {
            $vis trait $name: $($for)* {}
            impl<T: $($for)*> $name for T {}
        };
    }
}
