use crate::func::hashing::get_hasher;
use crate::{Identifier, ImmutableString};

#[cfg(feature = "no_std")]
use std::prelude::v1::*;
use std::{
    collections::BTreeMap,
    hash::{Hash, Hasher},
    marker::PhantomData,
    ops::AddAssign,
};

/// _(internals)_ A factory of identifiers from text strings.
/// Exported under the `internals` feature only.
///
/// Normal identifiers, property getters and setters are interned separately.
#[derive(Debug, Clone, Default, Hash)]
pub struct StringsInterner<'a> {
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

impl StringsInterner<'_> {
    /// Create a new [`StringsInterner`].
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
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
    pub fn get(&mut self, text: impl AsRef<str>) -> ImmutableString {
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
    pub fn get_with_prefix(
        &mut self,
        prefix: impl AsRef<str>,
        text: impl AsRef<str>,
    ) -> ImmutableString {
        let prefix = prefix.as_ref();
        let text = text.as_ref();

        let (dict, mapper): (_, fn(&str) -> Identifier) = match prefix {
            "" => (&mut self.strings, |s| s.into()),

            #[cfg(not(feature = "no_object"))]
            crate::engine::FN_GET => (&mut self.getters, crate::engine::make_getter),
            #[cfg(not(feature = "no_object"))]
            crate::engine::FN_SET => (&mut self.setters, crate::engine::make_setter),

            _ => unreachable!("unsupported prefix {}", prefix),
        };

        let hasher = &mut get_hasher();
        text.hash(hasher);
        let key = hasher.finish();

        if !dict.is_empty() && dict.contains_key(&key) {
            dict.get(&key).unwrap().clone()
        } else {
            let value: ImmutableString = mapper(text).into();
            dict.insert(key, value.clone());
            value
        }
    }

    /// Number of strings interned.
    #[inline(always)]
    #[must_use]
    pub fn len(&self) -> usize {
        #[cfg(not(feature = "no_object"))]
        return self.strings.len() + self.getters.len() + self.setters.len();

        #[cfg(feature = "no_object")]
        return self.strings.len();
    }

    /// Number of strings interned.
    #[inline(always)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        #[cfg(not(feature = "no_object"))]
        return self.strings.is_empty() || self.getters.is_empty() || self.setters.is_empty();

        #[cfg(feature = "no_object")]
        return self.strings.is_empty();
    }
}

impl AddAssign<Self> for StringsInterner<'_> {
    #[inline(always)]
    fn add_assign(&mut self, rhs: Self) {
        self.strings.extend(rhs.strings.into_iter());
        #[cfg(not(feature = "no_object"))]
        self.getters.extend(rhs.getters.into_iter());
        #[cfg(not(feature = "no_object"))]
        self.setters.extend(rhs.setters.into_iter());
    }
}

impl AddAssign<&Self> for StringsInterner<'_> {
    #[inline(always)]
    fn add_assign(&mut self, rhs: &Self) {
        self.strings
            .extend(rhs.strings.iter().map(|(k, v)| (k.clone(), v.clone())));
        #[cfg(not(feature = "no_object"))]
        self.getters
            .extend(rhs.getters.iter().map(|(k, v)| (k.clone(), v.clone())));
        #[cfg(not(feature = "no_object"))]
        self.setters
            .extend(rhs.setters.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
}
