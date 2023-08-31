//! Collection of custom types.

use crate::Identifier;
use std::{any::type_name, collections::BTreeMap};

/// _(internals)_ Information for a custom type.
/// Exported under the `internals` feature only.
#[derive(Debug, Eq, PartialEq, Clone, Hash, Default)]
pub struct CustomTypeInfo {
    /// Friendly display name of the custom type.
    pub display_name: Identifier,
    /// Comments.
    #[cfg(feature = "metadata")]
    pub comments: Box<[Identifier]>,
}

/// _(internals)_ A collection of custom types.
/// Exported under the `internals` feature only.
#[derive(Debug, Clone, Hash)]
pub struct CustomTypesCollection(BTreeMap<Identifier, CustomTypeInfo>);

impl Default for CustomTypesCollection {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

impl CustomTypesCollection {
    /// Create a new [`CustomTypesCollection`].
    #[inline(always)]
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }
    /// Register a custom type.
    #[inline(always)]
    pub fn add(&mut self, type_name: impl Into<Identifier>, name: impl Into<Identifier>) {
        self.add_raw(
            type_name,
            CustomTypeInfo {
                display_name: name.into(),
                #[cfg(feature = "metadata")]
                comments: Default::default(),
            },
        );
    }
    /// Register a custom type with doc-comments.
    /// Exported under the `metadata` feature only.
    #[cfg(feature = "metadata")]
    #[inline(always)]
    pub fn add_with_comments<C: Into<Identifier>>(
        &mut self,
        type_name: impl Into<Identifier>,
        name: impl Into<Identifier>,
        comments: impl IntoIterator<Item = C>,
    ) {
        self.add_raw(
            type_name,
            CustomTypeInfo {
                display_name: name.into(),
                comments: comments.into_iter().map(Into::into).collect(),
            },
        );
    }
    /// Register a custom type.
    #[inline(always)]
    pub fn add_type<T>(&mut self, name: &str) {
        self.add_raw(
            type_name::<T>(),
            CustomTypeInfo {
                display_name: name.into(),
                #[cfg(feature = "metadata")]
                comments: Default::default(),
            },
        );
    }
    /// Register a custom type with doc-comments.
    /// Exported under the `metadata` feature only.
    #[cfg(feature = "metadata")]
    #[inline(always)]
    pub fn add_type_with_comments<T>(&mut self, name: &str, comments: &[&str]) {
        self.add_raw(
            type_name::<T>(),
            CustomTypeInfo {
                display_name: name.into(),
                #[cfg(feature = "metadata")]
                comments: comments.iter().map(|&s| s.into()).collect(),
            },
        );
    }
    /// Register a custom type.
    #[inline(always)]
    pub fn add_raw(&mut self, type_name: impl Into<Identifier>, custom_type: CustomTypeInfo) {
        self.0.insert(type_name.into(), custom_type);
    }
    /// Find a custom type.
    #[inline(always)]
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&CustomTypeInfo> {
        self.0.get(key)
    }
    /// Iterate all the custom types.
    #[inline(always)]
    #[must_use]
    pub fn iter(&self) -> impl Iterator<Item = (&str, &CustomTypeInfo)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v))
    }
}
