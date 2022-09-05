//! Module containing utilities to hash functions and function calls.

#[cfg(feature = "no_std")]
use std::prelude::v1::*;
use std::{
    any::TypeId,
    hash::{BuildHasher, Hash, Hasher},
};

#[cfg(feature = "no_std")]
pub type StraightHashMap<K, V> = hashbrown::HashMap<K, V, StraightHasherBuilder>;
#[cfg(feature = "no_std")]
pub type StraightHashSet<K> = hashbrown::HashSet<K, StraightHasherBuilder>;

#[cfg(not(feature = "no_std"))]
pub type StraightHashMap<K, V> = std::collections::HashMap<K, V, StraightHasherBuilder>;
#[cfg(not(feature = "no_std"))]
pub type StraightHashSet<K> = std::collections::HashSet<K, StraightHasherBuilder>;

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
    fn finish(&self) -> u64 {
        self.0
    }
    #[inline]
    fn write(&mut self, _bytes: &[u8]) {
        panic!("StraightHasher can only hash u64 values");
    }

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
    fn build_hasher(&self) -> Self::Hasher {
        StraightHasher(ALT_ZERO_HASH)
    }
}

/// Create an instance of the default hasher.
#[inline(always)]
#[must_use]
pub fn get_hasher() -> ahash::AHasher {
    ahash::AHasher::default()
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
pub fn calc_qualified_var_hash<'a>(
    modules: impl IntoIterator<Item = &'a str>,
    var_name: &str,
) -> u64 {
    let s = &mut get_hasher();

    // We always skip the first module
    let mut len = 0;
    modules
        .into_iter()
        .inspect(|_| len += 1)
        .skip(1)
        .for_each(|m| m.hash(s));
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
pub fn calc_qualified_fn_hash<'a>(
    modules: impl IntoIterator<Item = &'a str>,
    fn_name: &str,
    num: usize,
) -> u64 {
    let s = &mut get_hasher();

    // We always skip the first module
    let mut len = 0;
    modules
        .into_iter()
        .inspect(|_| len += 1)
        .skip(1)
        .for_each(|m| m.hash(s));
    len.hash(s);
    fn_name.hash(s);
    num.hash(s);

    match s.finish() {
        0 => ALT_ZERO_HASH,
        r => r,
    }
}

/// Calculate a non-zero [`u64`] hash key from a non-namespace-qualified function name
/// and the number of parameters, but no parameter types.
///
/// Parameter types are passed in via [`TypeId`] values from an iterator.
///
/// # Zeros
///
/// If the hash happens to be zero, it is mapped to `DEFAULT_HASH`.
#[inline(always)]
#[must_use]
pub fn calc_fn_hash(fn_name: &str, num: usize) -> u64 {
    calc_qualified_fn_hash(None, fn_name, num)
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
pub fn calc_fn_params_hash(params: impl IntoIterator<Item = TypeId>) -> u64 {
    let s = &mut get_hasher();
    let mut len = 0;
    params.into_iter().for_each(|t| {
        len += 1;
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
