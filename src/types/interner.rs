use crate::func::{hashing::get_hasher, StraightHashMap};
use crate::ImmutableString;

#[cfg(feature = "no_std")]
use std::prelude::v1::*;
use std::{
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    ops::AddAssign,
};

/// Maximum number of strings interned.
pub const MAX_INTERNED_STRINGS: usize = 256;

/// Maximum length of strings interned.
pub const MAX_STRING_LEN: usize = 24;

/// _(internals)_ A factory of identifiers from text strings.
/// Exported under the `internals` feature only.
///
/// Normal identifiers, property getters and setters are interned separately.
#[derive(Clone)]
pub struct StringsInterner<'a> {
    /// Maximum number of strings interned.
    pub capacity: usize,
    /// Maximum string length.
    pub max_string_len: usize,
    /// Normal strings.
    strings: StraightHashMap<ImmutableString>,
    /// Take care of the lifetime parameter.
    dummy: PhantomData<&'a ()>,
}

impl Default for StringsInterner<'_> {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for StringsInterner<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.strings.values()).finish()
    }
}

impl StringsInterner<'_> {
    /// Create a new [`StringsInterner`].
    #[inline(always)]
    #[must_use]
    pub fn new() -> Self {
        Self {
            capacity: MAX_INTERNED_STRINGS,
            max_string_len: MAX_STRING_LEN,
            strings: StraightHashMap::default(),
            dummy: PhantomData,
        }
    }

    /// Get an identifier from a text string, adding it to the interner if necessary.
    #[inline(always)]
    #[must_use]
    pub fn get<S: AsRef<str> + Into<ImmutableString>>(&mut self, text: S) -> ImmutableString {
        self.get_with_mapper(Into::into, text)
    }

    /// Get an identifier from a text string, adding it to the interner if necessary.
    #[inline]
    #[must_use]
    pub fn get_with_mapper<S: AsRef<str>>(
        &mut self,
        mapper: fn(S) -> ImmutableString,
        text: S,
    ) -> ImmutableString {
        let key = text.as_ref();

        if key.len() > MAX_STRING_LEN {
            return mapper(text);
        }

        let hasher = &mut get_hasher();
        key.hash(hasher);
        let key = hasher.finish();

        if !self.strings.is_empty() && self.strings.contains_key(&key) {
            return self.strings.get(&key).unwrap().clone();
        }

        let value = mapper(text);

        if value.strong_count() > 1 {
            return value;
        }

        self.strings.insert(key, value.clone());

        // If the interner is over capacity, remove the longest entry
        if self.strings.len() > self.capacity {
            // Leave some buffer to grow when shrinking the cache.
            // We leave at least two entries, one for the empty string, and one for the string
            // that has just been inserted.
            let max = if self.capacity < 5 {
                2
            } else {
                self.capacity - 3
            };

            while self.strings.len() > max {
                let (_, n) = self.strings.iter().fold((0, 0), |(x, n), (&k, v)| {
                    if k != key && v.len() > x {
                        (v.len(), k)
                    } else {
                        (x, n)
                    }
                });

                self.strings.remove(&n);
            }
        }

        value
    }

    /// Number of strings interned.
    #[inline(always)]
    #[must_use]
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.strings.len()
    }

    /// Returns `true` if there are no interned strings.
    #[inline(always)]
    #[must_use]
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }

    /// Clear all interned strings.
    #[inline(always)]
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.strings.clear();
    }
}

impl AddAssign<Self> for StringsInterner<'_> {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Self) {
        self.strings.extend(rhs.strings.into_iter());
    }
}

impl AddAssign<&Self> for StringsInterner<'_> {
    #[inline(always)]
    fn add_assign(&mut self, rhs: &Self) {
        self.strings
            .extend(rhs.strings.iter().map(|(&k, v)| (k, v.clone())));
    }
}
