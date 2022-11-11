//! Module defining the AST (abstract syntax tree).

pub mod ast;
pub mod expr;
pub mod flags;
pub mod ident;
pub mod namespace;
pub mod script_fn;
pub mod stmt;

pub use ast::{ASTNode, AST};
#[cfg(not(feature = "no_custom_syntax"))]
pub use expr::CustomExpr;
pub use expr::{BinaryExpr, Expr, FnCallExpr, FnCallHashes};
pub use flags::{ASTFlags, FnAccess};
pub use ident::Ident;
#[cfg(not(feature = "no_module"))]
pub use namespace::Namespace;
#[cfg(not(feature = "no_module"))]
#[cfg(not(feature = "no_function"))]
pub use script_fn::EncapsulatedEnviron;
#[cfg(not(feature = "no_function"))]
pub use script_fn::{ScriptFnDef, ScriptFnMetadata};
pub use stmt::{
    CaseBlocksList, ConditionalExpr, OpAssignment, RangeCase, Stmt, StmtBlock, StmtBlockContainer,
    SwitchCasesCollection, TryCatchBlock,
};

/// _(internals)_ Placeholder for a script-defined function.
/// Exported under the `internals` feature only.
#[cfg(feature = "no_function")]
pub type ScriptFnDef = ();
