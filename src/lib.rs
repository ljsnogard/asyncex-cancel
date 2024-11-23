#![no_std]

// We always pull in `std` during tests, because it's just easier
// to write tests when you can assume you're on a capable platform
#[cfg(test)]
extern crate std;

mod source_;
mod token_;

#[cfg(test)]
mod tests_;

pub use source_::CancellationTokenSource;
pub use token_::CancellationToken;

pub mod x_deps {
    pub use mm_ptr;
    pub use spmv_oneshot;

    pub use spmv_oneshot::x_deps::{abs_sync, atomex, atomic_sync, pin_utils};
}