use crate::func::hashing::get_hasher;
use crate::ImmutableString;

#[cfg(feature = "no_std")]
use std::prelude::v1::*;
use std::{
    collections::BTreeMap,
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
#[derive(Debug, Clone, Hash)]
pub struct StringsInterner<'a> {
    /// Maximum capacity.
    max: usize,
    /// Normal strings.
    strings: BTreeMap<u64, ImmutableString>,
    /// Property getters.
    #[cfg(not(feature = "no_object"))]
    getters: BTreeMap<u64, ImmutableString>,
    /// Property setters.
    #[cfg(not(feature = "no_object"))]
    setters: BTreeMap<u64, ImmutableString>,
    /// Take care of the lifetime parameter.
    dummy: PhantomData<&'a ()>,
}

impl Default for StringsInterner<'_> {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

impl StringsInterner<'_> {
    /// Create a new [`StringsInterner`].
    #[inline(always)]
    #[must_use]
    pub fn new() -> Self {
        Self::new_with_capacity(MAX_INTERNED_STRINGS)
    }

    /// Create a new [`StringsInterner`] with a maximum capacity.
    #[inline]
    #[must_use]
    pub fn new_with_capacity(capacity: usize) -> Self {
        Self {
            max: capacity,
            strings: BTreeMap::new(),
            #[cfg(not(feature = "no_object"))]
            getters: BTreeMap::new(),
            #[cfg(not(feature = "no_object"))]
            setters: BTreeMap::new(),
            dummy: PhantomData,
        }
    }

    /// Get an identifier from a text string and prefix, adding it to the interner if necessary.
    #[inline(always)]
    #[must_use]
    pub fn get(&mut self, text: impl AsRef<str> + Into<ImmutableString>) -> ImmutableString {
        self.get_with_prefix("", text)
    }

    /// Get an identifier from a text string and prefix, adding it to the interner if necessary.
    ///
    /// # Prefix
    ///
    /// Currently recognized prefixes are:
    ///
    /// * `""` - None (normal string)
    /// * `"get$"` - Property getter, not available under `no_object`
    /// * `"set$"` - Property setter, not available under `no_object`
    ///
    /// # Panics
    ///
    /// Panics if the prefix is not recognized.
    #[inline]
    #[must_use]
    pub fn get_with_prefix<T: AsRef<str> + Into<ImmutableString>>(
        &mut self,
        prefix: impl AsRef<str>,
        text: T,
    ) -> ImmutableString {
        let prefix = prefix.as_ref();
        let key = text.as_ref();

        // Do not intern numbers
        if prefix == "" && key.bytes().all(|c| c == b'.' || (c >= b'0' && c <= b'9')) {
            return text.into();
        }

        let (dict, mapper): (_, fn(T) -> ImmutableString) = match prefix {
            "" => (&mut self.strings, |s| s.into()),

            #[cfg(not(feature = "no_object"))]
            crate::engine::FN_GET => (&mut self.getters, |s| {
                crate::engine::make_getter(s.as_ref()).into()
            }),
            #[cfg(not(feature = "no_object"))]
            crate::engine::FN_SET => (&mut self.setters, |s| {
                crate::engine::make_setter(s.as_ref()).into()
            }),

            _ => unreachable!("unsupported prefix {}", prefix),
        };

        if key.len() > MAX_STRING_LEN {
            return mapper(text);
        }

        let hasher = &mut get_hasher();
        key.hash(hasher);
        let key = hasher.finish();

        if !dict.is_empty() && dict.contains_key(&key) {
            return dict.get(&key).unwrap().clone();
        }

        let value = mapper(text);

        if value.strong_count() > 1 {
            return value;
        }

        dict.insert(key, value.clone());

        // If the interner is over capacity, remove the longest entry
        if self.strings.len() > self.max {
            // Leave some buffer to grow when shrinking the cache.
            // We leave at least two entries, one for the empty string, and one for the string
            // that has just been inserted.
            let max = if self.max < 5 { 2 } else { self.max - 3 };

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
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        #[cfg(not(feature = "no_object"))]
        return self.strings.len() + self.getters.len() + self.setters.len();

        #[cfg(feature = "no_object")]
        return self.strings.len();
    }

    /// Number of strings interned.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        #[cfg(not(feature = "no_object"))]
        return self.strings.is_empty() || self.getters.is_empty() || self.setters.is_empty();

        #[cfg(feature = "no_object")]
        return self.strings.is_empty();
    }

    /// Clear all interned strings.
    #[inline]
    pub fn clear(&mut self) {
        self.strings.clear();

        #[cfg(not(feature = "no_object"))]
        {
            self.getters.clear();
            self.setters.clear();
        }
    }
}

impl AddAssign<Self> for StringsInterner<'_> {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Self) {
        self.strings.extend(rhs.strings.into_iter());

        #[cfg(not(feature = "no_object"))]
        {
            self.getters.extend(rhs.getters.into_iter());
            self.setters.extend(rhs.setters.into_iter());
        }
    }
}

impl AddAssign<&Self> for StringsInterner<'_> {
    #[inline(always)]
    fn add_assign(&mut self, rhs: &Self) {
        self.strings
            .extend(rhs.strings.iter().map(|(&k, v)| (k, v.clone())));

        #[cfg(not(feature = "no_object"))]
        {
            self.getters
                .extend(rhs.getters.iter().map(|(&k, v)| (k, v.clone())));
            self.setters
                .extend(rhs.setters.iter().map(|(&k, v)| (k, v.clone())));
        }
    }
}
