//! AST generation and normalization for Solidity and Rust (Ink!/Anchor).
//!
//! Leaf crate of the MintCheck workspace: depends on no internal crate and
//! owns the normalized IR (see `PROJECT_SPEC.md` §4.1).

pub mod ir;

pub use ir::{ASTNode, NodeKind, SourceSpan};
