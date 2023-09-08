//! Collection of custom types.

use crate::Identifier;
#[cfg(feature = "no_std")]
use std::prelude::v1::*;
use std::{any::type_name, collections::BTreeMap};

/// Information for a registered custom type.
#[derive(Debug, Eq, PartialEq, Clone, Hash, Default)]
pub struct CustomTypeInfo {
    /// Rust name of the custom type.
    type_name: Identifier,
    /// Friendly display name of the custom type.
    display_name: Identifier,
    /// Comments.
    #[cfg(feature = "metadata")]
    comments: Box<[crate::SmartString]>,
}

impl CustomTypeInfo {
    /// Rust name of the custom type.
    #[inline(always)]
    #[must_use]
    pub fn type_name(&self) -> &str {
        &self.type_name
    }
    /// Friendly display name of the custom type.
    #[inline(always)]
    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
    /// _(metadata)_ Iterate the doc-comments defined on the custom type.
    /// Exported under the `metadata` feature only.
    #[cfg(feature = "metadata")]
    #[inline(always)]
    #[must_use]
    pub fn iter_comments(&self) -> impl Iterator<Item = &str> {
        self.comments.iter().map(|s| s.as_str())
    }
    /// _(metadata)_ Doc-comments defined on the custom type.
    /// Exported under the `metadata` feature only.
    #[cfg(feature = "metadata")]
    #[inline(always)]
    #[must_use]
    pub fn comments(&self) -> &[crate::SmartString] {
        &self.comments
    }
}

/// _(internals)_ A collection of custom types.
/// Exported under the `internals` feature only.
#[derive(Debug, Clone, Hash)]
pub struct CustomTypesCollection(BTreeMap<Identifier, Box<CustomTypeInfo>>);

impl Default for CustomTypesCollection {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

impl CustomTypesCollection {
    /// Create a new [`CustomTypesCollection`].
    #[inline(always)]
    pub const fn new() -> Self {
        Self(BTreeMap::new())
    }
    /// Clear the [`CustomTypesCollection`].
    #[inline(always)]
    pub fn clear(&mut self) {
        self.0.clear();
    }
    /// Register a custom type.
    #[inline(always)]
    pub fn add(&mut self, type_name: impl Into<Identifier>, name: impl Into<Identifier>) {
        let type_name = type_name.into();
        let custom_type = CustomTypeInfo {
            type_name: type_name.clone(),
            display_name: name.into(),
            #[cfg(feature = "metadata")]
            comments: <_>::default(),
        };
        self.add_raw(type_name, custom_type);
    }
    /// Register a custom type with doc-comments.
    /// Exported under the `metadata` feature only.
    #[cfg(feature = "metadata")]
    #[inline(always)]
    pub fn add_with_comments<C: Into<crate::SmartString>>(
        &mut self,
        type_name: impl Into<Identifier>,
        name: impl Into<Identifier>,
        comments: impl IntoIterator<Item = C>,
    ) {
        let type_name = type_name.into();
        let custom_type = CustomTypeInfo {
            type_name: type_name.clone(),
            display_name: name.into(),
            comments: comments.into_iter().map(Into::into).collect(),
        };
        self.add_raw(type_name, custom_type);
    }
    /// Register a custom type.
    #[inline(always)]
    pub fn add_type<T>(&mut self, name: &str) {
        self.add_raw(
            type_name::<T>(),
            CustomTypeInfo {
                type_name: type_name::<T>().into(),
                display_name: name.into(),
                #[cfg(feature = "metadata")]
                comments: <_>::default(),
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
                type_name: type_name::<T>().into(),
                display_name: name.into(),
                #[cfg(feature = "metadata")]
                comments: comments.iter().map(|&s| s.into()).collect(),
            },
        );
    }
    /// Register a custom type.
    #[inline(always)]
    pub fn add_raw(&mut self, type_name: impl Into<Identifier>, custom_type: CustomTypeInfo) {
        self.0.insert(type_name.into(), custom_type.into());
    }
    /// Find a custom type.
    #[inline(always)]
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&CustomTypeInfo> {
        self.0.get(key).map(<_>::as_ref)
    }
    /// Iterate all the custom types.
    #[inline(always)]
    pub fn iter(&self) -> impl Iterator<Item = (&str, &CustomTypeInfo)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_ref()))
    }
}
