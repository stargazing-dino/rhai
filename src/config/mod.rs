//! Configuration for Rhai.

mod hashing_env;
mod hashing_no_std;
mod hashing_std;

#[cfg(not(feature = "no_std"))]
pub use once_cell::sync::OnceCell as StaticCell;

#[cfg(feature = "no_std")]
pub use hashing_no_std::SusLock as StaticCell;

/// Fixed hashing seeds for stable hashing.
pub mod hashing {
    #[cfg(feature = "no_std")]
    pub use super::hashing_no_std::*;
    #[cfg(not(feature = "no_std"))]
    pub use super::hashing_std::*;
}
