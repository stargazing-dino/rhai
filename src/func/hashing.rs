//! Module containing utilities to hash functions and function calls.

use crate::config;
#[cfg(feature = "no_std")]
use std::prelude::v1::*;
use std::{
    any::TypeId,
    hash::{BuildHasher, Hash, Hasher},
};

#[cfg(feature = "no_std")]
pub type StraightHashMap<V> = hashbrown::HashMap<u64, V, StraightHasherBuilder>;

#[cfg(not(feature = "no_std"))]
pub type StraightHashMap<V> = std::collections::HashMap<u64, V, StraightHasherBuilder>;

/// Dummy hash value to map zeros to. This value can be anything.
///
/// # Notes
///
/// Hashes are `u64`, and they can be zero (although extremely unlikely).
/// It is possible to hijack the zero value to indicate non-existence,
/// like [`None`] in [`Option<u64>`].
///
/// When a hash is calculated to be zero, it gets mapped to this alternate hash value.
/// This has the effect of releasing the zero value at the expense of causing the probability of
/// this value to double, which has minor impacts.
pub const ALT_ZERO_HASH: u64 = 42;

/// A hasher that only takes one single [`u64`] and returns it as a non-zero hash key.
///
/// # Zeros
///
/// If the value is zero, it is mapped to `ALT_ZERO_HASH`.
///
/// # Panics
///
/// Panics when hashing any data type other than a [`u64`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct StraightHasher(u64);

impl Hasher for StraightHasher {
    #[inline(always)]
    #[must_use]
    fn finish(&self) -> u64 {
        self.0
    }
    #[inline(always)]
    fn write(&mut self, _bytes: &[u8]) {
        panic!("StraightHasher can only hash u64 values");
    }
    #[inline(always)]
    fn write_u64(&mut self, i: u64) {
        if i == 0 {
            self.0 = ALT_ZERO_HASH;
        } else {
            self.0 = i;
        }
    }
}

/// A hash builder for `StraightHasher`.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct StraightHasherBuilder;

impl BuildHasher for StraightHasherBuilder {
    type Hasher = StraightHasher;

    #[inline(always)]
    #[must_use]
    fn build_hasher(&self) -> Self::Hasher {
        StraightHasher(ALT_ZERO_HASH)
    }
}

/// Create an instance of the default hasher.
#[inline(always)]
#[must_use]
pub fn get_hasher() -> ahash::AHasher {
    match config::get_rhai_ahash_seed() {
        Some([seed1, seed2, seed3, seed4]) if seed1 | seed2 | seed3 | seed4 != 0 => {
            ahash::RandomState::with_seeds(seed1, seed2, seed3, seed4).build_hasher()
        }
        _ => ahash::AHasher::default(),
    }
}

/// Calculate a non-zero [`u64`] hash key from a namespace-qualified variable name.
///
/// Module names are passed in via `&str` references from an iterator.
/// Parameter types are passed in via [`TypeId`] values from an iterator.
///
/// # Zeros
///
/// If the hash happens to be zero, it is mapped to `DEFAULT_HASH`.
///
/// # Note
///
/// The first module name is skipped.  Hashing starts from the _second_ module in the chain.
#[inline]
#[must_use]
pub fn calc_var_hash<'a>(
    modules: impl IntoIterator<Item = &'a str, IntoIter = impl ExactSizeIterator<Item = &'a str>>,
    var_name: &str,
) -> u64 {
    let s = &mut get_hasher();

    // We always skip the first module
    let iter = modules.into_iter();
    let len = iter.len();
    iter.skip(1).for_each(|m| m.hash(s));
    len.hash(s);
    var_name.hash(s);

    match s.finish() {
        0 => ALT_ZERO_HASH,
        r => r,
    }
}

/// Calculate a non-zero [`u64`] hash key from a namespace-qualified function name
/// and the number of parameters, but no parameter types.
///
/// Module names making up the namespace are passed in via `&str` references from an iterator.
/// Parameter types are passed in via [`TypeId`] values from an iterator.
///
/// If the function is not namespace-qualified, pass [`None`] as the namespace.
///
/// # Zeros
///
/// If the hash happens to be zero, it is mapped to `DEFAULT_HASH`.
///
/// # Note
///
/// The first module name is skipped.  Hashing starts from the _second_ module in the chain.
#[inline]
#[must_use]
pub fn calc_fn_hash<'a>(
    namespace: impl IntoIterator<Item = &'a str, IntoIter = impl ExactSizeIterator<Item = &'a str>>,
    fn_name: &str,
    num: usize,
) -> u64 {
    let s = &mut get_hasher();

    // We always skip the first module
    let iter = namespace.into_iter();
    let len = iter.len();
    iter.skip(1).for_each(|m| m.hash(s));
    len.hash(s);
    fn_name.hash(s);
    num.hash(s);

    match s.finish() {
        0 => ALT_ZERO_HASH,
        r => r,
    }
}

/// Calculate a non-zero [`u64`] hash key from a list of parameter types.
///
/// Parameter types are passed in via [`TypeId`] values from an iterator.
///
/// # Zeros
///
/// If the hash happens to be zero, it is mapped to `DEFAULT_HASH`.
#[inline]
#[must_use]
pub fn calc_fn_params_hash(
    params: impl IntoIterator<Item = TypeId, IntoIter = impl ExactSizeIterator<Item = TypeId>>,
) -> u64 {
    let s = &mut get_hasher();
    let iter = params.into_iter();
    let len = iter.len();
    iter.for_each(|t| {
        t.hash(s);
    });
    len.hash(s);

    match s.finish() {
        0 => ALT_ZERO_HASH,
        r => r,
    }
}

/// Combine two [`u64`] hashes by taking the XOR of them.
///
/// # Zeros
///
/// If the hash happens to be zero, it is mapped to `DEFAULT_HASH`.
#[inline(always)]
#[must_use]
pub const fn combine_hashes(a: u64, b: u64) -> u64 {
    match a ^ b {
        0 => ALT_ZERO_HASH,
        r => r,
    }
}
