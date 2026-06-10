//! AST generation and normalization for Solidity and Rust (Ink!/Anchor).
//!
//! Leaf crate of the MintCheck workspace: depends on no internal crate and
//! owns the normalized `ASTNode` IR (see `PROJECT_SPEC.md` §4.1).

#[cfg(test)]
mod tests {
    #[test]
    fn test_scaffold() {
        assert_eq!(2 + 2, 4);
    }
}
