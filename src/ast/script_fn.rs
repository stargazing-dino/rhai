//! Module defining script-defined functions.
#![cfg(not(feature = "no_function"))]

use super::{FnAccess, StmtBlock};
use crate::{FnArgsVec, ImmutableString};
#[cfg(feature = "no_std")]
use std::prelude::v1::*;
use std::{fmt, hash::Hash};

/// _(internals)_ Encapsulated AST environment.
/// Exported under the `internals` feature only.
///
/// 1) other functions defined within the same AST
/// 2) the stack of imported [modules][crate::Module]
/// 3) global constants
///
/// Not available under `no_module` or `no_function`.
#[cfg(not(feature = "no_module"))]
#[derive(Debug, Clone)]
pub struct EncapsulatedEnviron {
    /// Functions defined within the same [`AST`][crate::AST].
    pub lib: crate::SharedModule,
    /// Imported [modules][crate::Module].
    pub imports: Box<[(ImmutableString, crate::SharedModule)]>,
    /// Globally-defined constants.
    pub constants: Option<crate::eval::SharedGlobalConstants>,
}

/// _(internals)_ A type containing information on a script-defined function.
/// Exported under the `internals` feature only.
#[derive(Debug, Clone)]
pub struct ScriptFnDef {
    /// Function body.
    pub body: StmtBlock,
    /// Encapsulated AST environment, if any.
    #[cfg(not(feature = "no_module"))]
    pub environ: Option<crate::Shared<EncapsulatedEnviron>>,
    /// Function name.
    pub name: ImmutableString,
    /// Function access mode.
    pub access: FnAccess,
    /// Names of function parameters.
    pub params: FnArgsVec<ImmutableString>,
    /// _(metadata)_ Function doc-comments (if any).
    /// Exported under the `metadata` feature only.
    ///
    /// Doc-comments are comment lines beginning with `///` or comment blocks beginning with `/**`,
    /// placed immediately before a function definition.
    ///
    /// Block doc-comments are kept in a single string slice with line-breaks within.
    ///
    /// Line doc-comments are merged, with line-breaks, into a single string slice without a termination line-break.
    ///
    /// Leading white-spaces are stripped, and each string slice always starts with the
    /// corresponding doc-comment leader: `///` or `/**`.
    ///
    /// Each line in non-block doc-comments starts with `///`.
    #[cfg(feature = "metadata")]
    pub comments: Box<[Box<str>]>,
}

impl fmt::Display for ScriptFnDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}({})",
            match self.access {
                FnAccess::Public => "",
                FnAccess::Private => "private ",
            },
            self.name,
            self.params
                .iter()
                .map(|s| s.as_str())
                .collect::<FnArgsVec<_>>()
                .join(", ")
        )
    }
}

/// A type containing the metadata of a script-defined function.
///
/// Not available under `no_function`.
///
/// Created by [`AST::iter_functions`][super::AST::iter_functions].
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone, Hash)]
#[non_exhaustive]
pub struct ScriptFnMetadata<'a> {
    /// Function name.
    pub name: &'a str,
    /// Function parameters (if any).
    pub params: Vec<&'a str>,
    /// Function access mode.
    pub access: FnAccess,
    /// _(metadata)_ Function doc-comments (if any).
    /// Exported under the `metadata` feature only.
    ///
    /// Doc-comments are comment lines beginning with `///` or comment blocks beginning with `/**`,
    /// placed immediately before a function definition.
    ///
    /// Block doc-comments are kept in a single string slice with line-breaks within.
    ///
    /// Line doc-comments are merged, with line-breaks, into a single string slice without a termination line-break.
    ///
    /// Leading white-spaces are stripped, and each string slice always starts with the
    /// corresponding doc-comment leader: `///` or `/**`.
    ///
    /// Each line in non-block doc-comments starts with `///`.
    #[cfg(feature = "metadata")]
    pub comments: Vec<&'a str>,
}

impl fmt::Display for ScriptFnMetadata<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}({})",
            match self.access {
                FnAccess::Public => "",
                FnAccess::Private => "private ",
            },
            self.name,
            self.params
                .iter()
                .copied()
                .collect::<FnArgsVec<_>>()
                .join(", ")
        )
    }
}

impl<'a> From<&'a ScriptFnDef> for ScriptFnMetadata<'a> {
    #[inline]
    fn from(value: &'a ScriptFnDef) -> Self {
        Self {
            name: &value.name,
            params: value.params.iter().map(|s| s.as_str()).collect(),
            access: value.access,
            #[cfg(feature = "metadata")]
            comments: value.comments.iter().map(<_>::as_ref).collect(),
        }
    }
}
