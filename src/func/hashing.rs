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
/// A hasher that only takes one single [`u64`] and returns it as a hash key.
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
        self.0 = i;
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
        StraightHasher(0)
    }
}

/// Create an instance of the default hasher.
#[inline(always)]
#[must_use]
pub fn get_hasher() -> ahash::AHasher {
    match config::hashing::get_ahash_seed() {
        Some([seed1, seed2, seed3, seed4]) if (seed1 | seed2 | seed3 | seed4) != 0 => {
            ahash::RandomState::with_seeds(*seed1, *seed2, *seed3, *seed4).build_hasher()
        }
        _ => ahash::AHasher::default(),
    }
}

/// Calculate a [`u64`] hash key from a namespace-qualified variable name.
///
/// Module names are passed in via `&str` references from an iterator.
/// Parameter types are passed in via [`TypeId`] values from an iterator.
///
/// # Note
///
/// The first module name is skipped.  Hashing starts from the _second_ module in the chain.
#[inline]
#[must_use]
pub fn calc_var_hash<'a>(namespace: impl IntoIterator<Item = &'a str>, var_name: &str) -> u64 {
    let s = &mut get_hasher();
    let mut count = 0;

    // We always skip the first module
    namespace.into_iter().for_each(|m| {
        // We always skip the first module
        if count > 0 {
            m.hash(s);
        }
        count += 1;
    });
    count.hash(s);
    var_name.hash(s);

    s.finish()
}

/// Calculate a [`u64`] hash key from a namespace-qualified function name
/// and the number of parameters, but no parameter types.
///
/// Module names making up the namespace are passed in via `&str` references from an iterator.
/// Parameter types are passed in via [`TypeId`] values from an iterator.
///
/// If the function is not namespace-qualified, pass [`None`] as the namespace.
///
/// # Note
///
/// The first module name is skipped.  Hashing starts from the _second_ module in the chain.
#[inline]
#[must_use]
pub fn calc_fn_hash<'a>(
    namespace: impl IntoIterator<Item = &'a str>,
    fn_name: &str,
    num: usize,
) -> u64 {
    let s = &mut get_hasher();
    let mut count = 0;

    namespace.into_iter().for_each(|m| {
        // We always skip the first module
        if count > 0 {
            m.hash(s);
        }
        count += 1;
    });
    count.hash(s);
    fn_name.hash(s);
    num.hash(s);

    s.finish()
}

/// Calculate a [`u64`] hash key from a base [`u64`] hash key and a list of parameter types.
///
/// Parameter types are passed in via [`TypeId`] values from an iterator.
#[inline]
#[must_use]
pub fn calc_fn_hash_full(base: u64, params: impl IntoIterator<Item = TypeId>) -> u64 {
    let s = &mut get_hasher();
    base.hash(s);
    let mut count = 0;
    params.into_iter().for_each(|t| {
        t.hash(s);
        count += 1;
    });
    count.hash(s);

    s.finish()
}
