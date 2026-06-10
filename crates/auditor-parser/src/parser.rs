//! Deterministic parser for Solidity-like sources, producing normalized IR
//! trees. A single-pass keyword/brace scanner: it recognizes `contract`
//! definitions, `function` members, and state-variable declarations, mapping
//! each to precise byte-offset spans. It does not evaluate expressions and
//! treats braces inside string literals as structural (a documented
//! limitation of this deterministic engine).

use std::error::Error;
use std::fmt;

use crate::ir::{ASTNode, NodeKind, SourceSpan};

/// Failure states of the parser engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The input contained nothing but whitespace.
    EmptySource,
    /// The input was structurally invalid; the message names the problem.
    SyntaxError(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::EmptySource => f.write_str("source text is empty"),
            ParseError::SyntaxError(msg) => write!(f, "syntax error: {msg}"),
        }
    }
}

impl Error for ParseError {}

/// Elementary type keywords that begin a state-variable declaration when
/// they appear at contract-body depth.
const TYPE_KEYWORDS: [&str; 12] = [
    "uint256", "uint128", "uint64", "uint8", "uint", "int256", "int", "address", "bool", "bytes32",
    "bytes", "mapping",
];

/// Deterministic Solidity source parser. Stateless; one engine can parse
/// any number of sources.
#[derive(Debug, Default)]
pub struct ParserEngine;

impl ParserEngine {
    pub fn new() -> Self {
        Self
    }

    /// Parses Solidity-like source text into a normalized AST rooted at a
    /// `SourceUnit` node spanning the entire input.
    pub fn parse_solidity(&self, source: &str) -> Result<ASTNode, ParseError> {
        if source.trim().is_empty() {
            return Err(ParseError::EmptySource);
        }
        let mut next_id = 0;
        let mut root = ASTNode::new(
            take_id(&mut next_id),
            NodeKind::SourceUnit,
            SourceSpan::new(0, source.len()),
        );
        let mut cursor = 0;
        while let Some(kw_start) = find_keyword(source, "contract", cursor) {
            let contract = parse_contract(source, kw_start, &mut next_id)?;
            cursor = contract.span.end;
            root.children.push(contract);
        }
        if root.children.is_empty() {
            return Err(ParseError::SyntaxError(
                "no contract definition found".to_owned(),
            ));
        }
        Ok(root)
    }
}

fn take_id(next_id: &mut usize) -> usize {
    let id = *next_id;
    *next_id += 1;
    id
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// True when `keyword` occurs at `at` on identifier boundaries.
fn keyword_at(source: &str, at: usize, keyword: &str) -> bool {
    let Some(rest) = source.get(at..) else {
        return false;
    };
    let bytes = source.as_bytes();
    rest.starts_with(keyword)
        && (at == 0 || !is_ident_byte(bytes[at - 1]))
        && bytes
            .get(at + keyword.len())
            .is_none_or(|b| !is_ident_byte(*b))
}

/// Next boundary-respecting occurrence of `keyword` at or after `from`.
fn find_keyword(source: &str, keyword: &str, from: usize) -> Option<usize> {
    let mut cursor = from;
    while let Some(rel) = source.get(cursor..)?.find(keyword) {
        let at = cursor + rel;
        if keyword_at(source, at, keyword) {
            return Some(at);
        }
        cursor = at + keyword.len();
    }
    None
}

/// Index of the `}` matching the `{` at `open`, scanning by brace depth.
fn find_matching_brace(source: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (i, b) in source.bytes().enumerate().skip(open) {
        match b {
            b'{' => depth += 1,
            b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_contract(source: &str, start: usize, next_id: &mut usize) -> Result<ASTNode, ParseError> {
    let after_kw = start + "contract".len();
    let name: String = source[after_kw..]
        .trim_start()
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        return Err(ParseError::SyntaxError(
            "expected contract name after 'contract' keyword".to_owned(),
        ));
    }
    let open = source[after_kw..]
        .find('{')
        .map(|i| after_kw + i)
        .ok_or_else(|| ParseError::SyntaxError(format!("expected '{{' after contract '{name}'")))?;
    let close = find_matching_brace(source, open).ok_or_else(|| {
        ParseError::SyntaxError(format!("unbalanced braces in contract '{name}'"))
    })?;
    let mut node = ASTNode::new(
        take_id(next_id),
        NodeKind::ContractDefinition,
        SourceSpan::new(start, close + 1),
    );
    parse_members(source, open + 1, close, next_id, &mut node)?;
    Ok(node)
}

/// Scans a contract body for members at body depth: functions and
/// state-variable declarations, in source order.
fn parse_members(
    source: &str,
    body_start: usize,
    body_end: usize,
    next_id: &mut usize,
    contract: &mut ASTNode,
) -> Result<(), ParseError> {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut i = body_start;
    while i < body_end {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => {
                if keyword_at(source, i, "function") {
                    let function = parse_function(source, i, next_id)?;
                    i = function.span.end;
                    contract.children.push(function);
                    continue;
                }
                if TYPE_KEYWORDS.iter().any(|kw| keyword_at(source, i, kw)) {
                    let semi = source[i..body_end]
                        .find(';')
                        .map(|j| i + j)
                        .ok_or_else(|| {
                            ParseError::SyntaxError(
                                "unterminated state variable declaration".to_owned(),
                            )
                        })?;
                    contract.children.push(ASTNode::new(
                        take_id(next_id),
                        NodeKind::VariableDeclaration,
                        SourceSpan::new(i, semi + 1),
                    ));
                    i = semi + 1;
                    continue;
                }
            }
            _ => {}
        }
        i += 1;
    }
    Ok(())
}

/// Parses one function: from the `function` keyword through its body's
/// closing brace (or `;` for a bodyless declaration). Each `;`-terminated
/// statement at body depth becomes an `Expression` child node.
fn parse_function(source: &str, start: usize, next_id: &mut usize) -> Result<ASTNode, ParseError> {
    let sig_end = source[start..]
        .find(['{', ';'])
        .map(|i| start + i)
        .ok_or_else(|| ParseError::SyntaxError("unterminated function definition".to_owned()))?;
    if source.as_bytes()[sig_end] != b'{' {
        let span = SourceSpan::new(start, sig_end + 1);
        return Ok(ASTNode::new(
            take_id(next_id),
            NodeKind::FunctionDefinition,
            span,
        ));
    }
    let close = find_matching_brace(source, sig_end)
        .ok_or_else(|| ParseError::SyntaxError("unbalanced braces in function body".to_owned()))?;
    let mut node = ASTNode::new(
        take_id(next_id),
        NodeKind::FunctionDefinition,
        SourceSpan::new(start, close + 1),
    );

    let mut depth = 0usize;
    let mut stmt_start: Option<usize> = None;
    for (i, &b) in source
        .as_bytes()
        .iter()
        .enumerate()
        .take(close)
        .skip(sig_end + 1)
    {
        match b {
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            b';' if depth == 0 => {
                if let Some(s) = stmt_start.take() {
                    node.children.push(ASTNode::new(
                        take_id(next_id),
                        NodeKind::Expression,
                        SourceSpan::new(s, i + 1),
                    ));
                }
            }
            b if depth == 0 && stmt_start.is_none() && !b.is_ascii_whitespace() => {
                stmt_start = Some(i);
            }
            _ => {}
        }
    }
    Ok(node)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "contract Vault {\n    uint256 balance;\n    address public owner;\n\n    function deposit() public {\n        balance = balance + 1;\n    }\n\n    function getOwner() public view returns (address) {\n        return owner;\n    }\n}\n";

    #[test]
    fn parses_contract_with_nested_members_in_source_order() {
        let root = ParserEngine::new()
            .parse_solidity(SRC)
            .expect("valid source must parse");
        assert_eq!(root.kind, NodeKind::SourceUnit);
        assert_eq!(root.span, SourceSpan::new(0, SRC.len()));
        assert_eq!(root.children.len(), 1);

        let contract = &root.children[0];
        assert_eq!(contract.kind, NodeKind::ContractDefinition);
        let kinds: Vec<NodeKind> = contract.children.iter().map(|c| c.kind).collect();
        assert_eq!(
            kinds,
            [
                NodeKind::VariableDeclaration,
                NodeKind::VariableDeclaration,
                NodeKind::FunctionDefinition,
                NodeKind::FunctionDefinition,
            ]
        );
    }

    #[test]
    fn spans_match_exact_byte_offsets_in_source() {
        let root = ParserEngine::new()
            .parse_solidity(SRC)
            .expect("valid source must parse");
        let contract = &root.children[0];

        let close = SRC.rfind('}').expect("closing brace present");
        assert_eq!(contract.span, SourceSpan::new(0, close + 1));

        let balance_start = SRC.find("uint256 balance").expect("decl present");
        assert_eq!(
            contract.children[0].span,
            SourceSpan::new(balance_start, balance_start + "uint256 balance;".len())
        );

        let deposit_start = SRC.find("function deposit").expect("fn present");
        let deposit_close =
            SRC.find("+ 1;\n    }").expect("body end present") + "+ 1;\n    }".len();
        assert_eq!(
            contract.children[2].span,
            SourceSpan::new(deposit_start, deposit_close)
        );
        let text = &SRC[deposit_start..deposit_close];
        assert!(text.starts_with("function deposit"));
        assert!(text.ends_with('}'));
    }

    #[test]
    fn function_bodies_yield_expression_statement_children() {
        let root = ParserEngine::new()
            .parse_solidity(SRC)
            .expect("valid source must parse");
        let contract = &root.children[0];

        let deposit = &contract.children[2];
        assert_eq!(deposit.children.len(), 1);
        let stmt = &deposit.children[0];
        assert_eq!(stmt.kind, NodeKind::Expression);
        let stmt_start = SRC.find("balance = balance + 1;").expect("stmt present");
        assert_eq!(
            stmt.span,
            SourceSpan::new(stmt_start, stmt_start + "balance = balance + 1;".len())
        );

        let get_owner = &contract.children[3];
        assert_eq!(get_owner.children.len(), 1);
        assert_eq!(
            &SRC[get_owner.children[0].span.start..get_owner.children[0].span.end],
            "return owner;"
        );
    }

    #[test]
    fn assigns_unique_preorder_node_ids() {
        let root = ParserEngine::new()
            .parse_solidity(SRC)
            .expect("valid source must parse");
        let mut ids = Vec::new();
        root.walk(&mut |n| ids.push(n.id));
        assert_eq!(ids, (0..root.subtree_size()).collect::<Vec<_>>());
    }

    #[test]
    fn parses_multiple_contracts_per_source_unit() {
        let src = "contract A { uint x; }\ncontract B { function f() external; }";
        let root = ParserEngine::new()
            .parse_solidity(src)
            .expect("valid source must parse");
        assert_eq!(root.children.len(), 2);
        let b = &root.children[1];
        let b_start = src.find("contract B").expect("B present");
        assert_eq!(b.span, SourceSpan::new(b_start, src.len()));
        assert_eq!(b.children[0].kind, NodeKind::FunctionDefinition);
    }

    #[test]
    fn empty_source_is_rejected() {
        let engine = ParserEngine::new();
        assert_eq!(engine.parse_solidity(""), Err(ParseError::EmptySource));
        assert_eq!(
            engine.parse_solidity("  \n\t"),
            Err(ParseError::EmptySource)
        );
    }

    #[test]
    fn malformed_input_yields_syntax_errors() {
        let engine = ParserEngine::new();

        let unbalanced = "contract Vault {\n    function f() public {\n}";
        assert!(matches!(
            engine.parse_solidity(unbalanced),
            Err(ParseError::SyntaxError(msg)) if msg.contains("unbalanced")
        ));

        let unnamed = "contract { uint x; }";
        assert!(matches!(
            engine.parse_solidity(unnamed),
            Err(ParseError::SyntaxError(msg)) if msg.contains("expected contract name")
        ));

        let no_contract = "uint x;";
        assert!(matches!(
            engine.parse_solidity(no_contract),
            Err(ParseError::SyntaxError(msg)) if msg.contains("no contract definition")
        ));
    }

    #[test]
    fn parse_error_displays_cleanly() {
        assert_eq!(ParseError::EmptySource.to_string(), "source text is empty");
        assert_eq!(
            ParseError::SyntaxError("boom".to_owned()).to_string(),
            "syntax error: boom"
        );
    }
}
