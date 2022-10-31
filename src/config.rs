//! Fixed hashing seeds for stable hashing.
//!
//! Set to [`None`] to disable stable hashing.
//!
//! See [`set_rhai_ahash_seed`] for more.
//!
//! Alternatively, You can also set this at compile time by disabling `stable_hash` and setting the `RHAI_AHASH_SEED`
//! environment variable instead.
//!
//! E.g. `env RHAI_AHASH_SEED ="[236,800,954,213]"`
// [236,800,954,213], haha funny yume nikki reference epic uboachan face numberworld nexus moment 100

#[cfg(feature = "stable_hash")]
use std::sync::OnceLock;

#[cfg(not(feature = "stable_hash"))]
const AHASH_SEED: Option<[u64; 4]> = None;
#[cfg(feature = "stable_hash")]
static AHASH_SEED: OnceLock<Option<[u64; 4]>> = OnceLock::new();

#[cfg(feature = "stable_hash")]
// #[doc(cfg(feature = "stable_hash"))]
/// Sets the Rhai Ahash seed. This is used to hash functions and the like.
///
/// This is a global variable, and thus will affect every Rhai instance.
/// This should not be used _unless_ you know you need it.
///
/// **WARNING**:
/// - You can only call this function **ONCE** for the whole of your program execution.
/// - You should gracefully handle the `Err(())`.
/// - You **MUST** call this before **ANY** Rhai operation occurs (e.g. creating an [`Engine`]).
///
/// # Errors
/// This will error if the AHashSeed is already set.
pub fn set_rhai_ahash_seed(new_seed: Option<[u64; 4]>) -> Result<(), Option<[u64; 4]>> {
    AHASH_SEED.set(new_seed)
}

#[cfg(feature = "stable_hash")]
/// Gets the current Rhai Ahash Seed.
///
/// See [`set_rhai_ahash_seed`] for more.
pub fn get_rhai_ahash_seed() -> Option<[u64; 4]> {
    AHASH_SEED.get().map(|x| *x).flatten()
}

#[cfg(not(feature = "stable_hash"))]
/// Gets the current Rhai Ahash Seed.
///
/// See [`AHASH_SEED`] and [`set_rhai_ahash_seed`] for more.
pub fn get_rhai_ahash_seed() -> Option<[u64; 4]> {
    AHASH_SEED
}
