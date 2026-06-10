//! SWC-115: authorization through `tx.origin`. Walks the normalized AST and
//! flags every `Expression` node whose source text contains a `tx.origin`
//! usage. The detector holds the file's source so it can resolve a node's
//! byte span back to text (the simplified IR carries no expression payload).

use auditor_parser::ir::{ASTNode, NodeKind};

use crate::finding::{Severity, Vulnerability};
use crate::Detector;

pub struct TxOriginDetector {
    /// Source text of the file the AST under analysis was parsed from.
    source: String,
}

impl TxOriginDetector {
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
        }
    }
}

impl Detector for TxOriginDetector {
    fn name(&self) -> &'static str {
        "Authorization via tx.origin"
    }

    fn id(&self) -> &'static str {
        "SWC-115"
    }

    fn severity(&self) -> Severity {
        Severity::High
    }

    fn check(&self, node: &ASTNode, file_path: &str) -> Vec<Vulnerability> {
        let mut findings = Vec::new();
        node.walk(&mut |n| {
            if n.kind != NodeKind::Expression {
                return;
            }
            let Some(text) = self.source.get(n.span.start..n.span.end) else {
                return;
            };
            if text.contains("tx.origin") {
                findings.push(Vulnerability {
                    id: self.id().to_owned(),
                    title: self.name().to_owned(),
                    description: "tx.origin is the transaction originator, not the \
                                  immediate caller; a phishing contract can relay calls \
                                  that pass this check. Use msg.sender for authorization."
                        .to_owned(),
                    severity: self.severity(),
                    span: n.span,
                    file_path: file_path.to_owned(),
                });
            }
        });
        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use auditor_parser::ir::SourceSpan;

    const SRC: &str =
        "contract Phish { function auth() public { require(tx.origin == owner); count = count + 1; } }";

    /// Mock AST: a contract holding one function with two expressions —
    /// the dangerous tx.origin check and a benign counter increment.
    fn mock_ast() -> (ASTNode, SourceSpan) {
        let danger_start = SRC.find("tx.origin == owner").expect("present");
        let danger = SourceSpan::new(danger_start, danger_start + "tx.origin == owner".len());
        let benign_start = SRC.find("count = count + 1").expect("present");
        let benign = SourceSpan::new(benign_start, benign_start + "count = count + 1".len());

        let function = ASTNode::new(2, NodeKind::FunctionDefinition, SourceSpan::new(17, 93))
            .with_child(ASTNode::new(3, NodeKind::Expression, danger))
            .with_child(ASTNode::new(4, NodeKind::Expression, benign));
        let root = ASTNode::new(0, NodeKind::SourceUnit, SourceSpan::new(0, SRC.len())).with_child(
            ASTNode::new(
                1,
                NodeKind::ContractDefinition,
                SourceSpan::new(0, SRC.len()),
            )
            .with_child(function),
        );
        (root, danger)
    }

    #[test]
    fn flags_exactly_one_tx_origin_expression_as_high() {
        let (root, danger_span) = mock_ast();
        let detector = TxOriginDetector::new(SRC);
        let findings = detector.check(&root, "contracts/Phish.sol");

        assert_eq!(findings.len(), 1);
        let v = &findings[0];
        assert_eq!(v.severity, Severity::High);
        assert_eq!(v.id, "SWC-115");
        assert_eq!(v.span, danger_span);
        assert_eq!(v.file_path, "contracts/Phish.sol");
        assert!(!v.description.is_empty());
    }

    #[test]
    fn clean_ast_yields_no_findings() {
        let src = "contract Safe { function f() public { count = count + 1; } }";
        let expr_start = src.find("count = count + 1").expect("present");
        let root = ASTNode::new(0, NodeKind::SourceUnit, SourceSpan::new(0, src.len())).with_child(
            ASTNode::new(
                1,
                NodeKind::Expression,
                SourceSpan::new(expr_start, expr_start + "count = count + 1".len()),
            ),
        );
        assert!(TxOriginDetector::new(src)
            .check(&root, "contracts/Safe.sol")
            .is_empty());
    }

    #[test]
    fn non_expression_nodes_are_never_flagged() {
        // The whole function span contains "tx.origin", but only Expression
        // nodes are eligible — a tree without them must stay silent.
        let (mut root, _) = mock_ast();
        root.children[0].children[0].children.clear();
        assert!(TxOriginDetector::new(SRC)
            .check(&root, "contracts/Phish.sol")
            .is_empty());
    }
}
