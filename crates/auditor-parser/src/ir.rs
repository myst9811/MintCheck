//! Normalized intermediate representation (IR) shared by all language
//! frontends. Every parser lowers its language-specific parse tree into
//! these types; detectors operate exclusively on them.

use serde::{Deserialize, Serialize};

/// Byte-offset range of a node in its source file (half-open: `[start, end)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
}

impl SourceSpan {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Length of the spanned region in bytes.
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Fundamental smart contract constructs, unified across source languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    SourceUnit,
    ContractDefinition,
    FunctionDefinition,
    VariableDeclaration,
    Statement,
    Expression,
}

/// A node in the normalized AST: a unique id, its construct kind, its
/// source location, and its child nodes in source order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ASTNode {
    pub id: usize,
    pub kind: NodeKind,
    pub span: SourceSpan,
    pub children: Vec<ASTNode>,
}

impl ASTNode {
    /// Creates a leaf node (no children).
    pub fn new(id: usize, kind: NodeKind, span: SourceSpan) -> Self {
        Self {
            id,
            kind,
            span,
            children: Vec::new(),
        }
    }

    /// Builder-style helper to attach a child node.
    #[must_use]
    pub fn with_child(mut self, child: ASTNode) -> Self {
        self.children.push(child);
        self
    }

    /// Total number of nodes in this subtree, including `self`.
    pub fn subtree_size(&self) -> usize {
        1 + self
            .children
            .iter()
            .map(ASTNode::subtree_size)
            .sum::<usize>()
    }

    /// Depth-first pre-order traversal over the subtree rooted at `self`.
    pub fn walk(&self, visit: &mut impl FnMut(&ASTNode)) {
        visit(self);
        for child in &self.children {
            child.walk(visit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock tree mirroring:
    /// `contract C { uint x; function f() { x = 1; } }`
    fn mock_ast() -> ASTNode {
        let state_var = ASTNode::new(2, NodeKind::VariableDeclaration, SourceSpan::new(13, 20));
        let assignment = ASTNode::new(4, NodeKind::Expression, SourceSpan::new(38, 43));
        let body_stmt =
            ASTNode::new(3, NodeKind::Statement, SourceSpan::new(38, 44)).with_child(assignment);
        let function = ASTNode::new(1, NodeKind::FunctionDefinition, SourceSpan::new(22, 46))
            .with_child(body_stmt);
        ASTNode::new(0, NodeKind::ContractDefinition, SourceSpan::new(0, 48))
            .with_child(state_var)
            .with_child(function)
    }

    #[test]
    fn child_traversal_visits_all_nodes_in_preorder() {
        let root = mock_ast();
        assert_eq!(root.subtree_size(), 5);

        let mut visited = Vec::new();
        root.walk(&mut |node| visited.push((node.id, node.kind)));
        assert_eq!(
            visited,
            vec![
                (0, NodeKind::ContractDefinition),
                (2, NodeKind::VariableDeclaration),
                (1, NodeKind::FunctionDefinition),
                (3, NodeKind::Statement),
                (4, NodeKind::Expression),
            ]
        );
    }

    #[test]
    fn direct_children_are_ordered_and_typed() {
        let root = mock_ast();
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.children[0].kind, NodeKind::VariableDeclaration);
        assert_eq!(root.children[1].kind, NodeKind::FunctionDefinition);
        // The function holds the statement, which holds the expression.
        let function = &root.children[1];
        assert_eq!(function.children[0].kind, NodeKind::Statement);
        assert_eq!(function.children[0].children[0].kind, NodeKind::Expression);
    }

    #[test]
    fn source_span_length_and_emptiness() {
        assert_eq!(SourceSpan::new(10, 25).len(), 15);
        assert!(SourceSpan::new(7, 7).is_empty());
        assert!(!SourceSpan::new(0, 1).is_empty());
    }

    #[test]
    fn json_round_trip_preserves_structure() {
        let root = mock_ast();
        let json = serde_json::to_string_pretty(&root).expect("serialization must succeed");

        // Spot-check the wire format is human-meaningful.
        assert!(json.contains("\"ContractDefinition\""));
        assert!(json.contains("\"FunctionDefinition\""));
        assert!(json.contains("\"start\": 38"));

        let restored: ASTNode = serde_json::from_str(&json).expect("deserialization must succeed");
        assert_eq!(restored, root);
        assert_eq!(restored.subtree_size(), 5);
    }

    #[test]
    fn json_deserializes_from_externally_authored_payload() {
        let payload = r#"{
            "id": 9,
            "kind": "SourceUnit",
            "span": { "start": 0, "end": 100 },
            "children": [
                { "id": 10, "kind": "ContractDefinition",
                  "span": { "start": 0, "end": 100 }, "children": [] }
            ]
        }"#;
        let node: ASTNode = serde_json::from_str(payload).expect("payload must parse");
        assert_eq!(node.id, 9);
        assert_eq!(node.kind, NodeKind::SourceUnit);
        assert_eq!(node.children.len(), 1);
        assert_eq!(node.children[0].kind, NodeKind::ContractDefinition);
        assert_eq!(node.children[0].span, SourceSpan::new(0, 100));
    }
}
