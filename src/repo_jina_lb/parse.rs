//! Tree-sitter-backed structural chunking used as the ingestion boundary for embeddings.
//!
//! SCIP ingestion is deliberately **not** wired here yet. The practical workflow is still:
//!
//! ```text
//! (optional external) compiler language server → emits index.scip
//! ```
//!
//! Future work can map SCIP occurrences into `CodeUnit`s before calling Jina, but browsers should not be blocked.

use std::path::Path;
use tree_sitter::Parser;

#[derive(Clone, Debug)]
pub(crate) struct CodeUnit {
    pub target_table: &'static str,
    pub target_kind: &'static str,
    pub signature: String,
    pub qualified_name: String,
    pub name_line: usize,
    /// UTF-8 source slice used for BOTH embedding payload and deterministic chunk id (`_chunk_id`).
    pub source_text: String,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum LangPack {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Tsx,
}

fn posix_rel(path: &str) -> String {
    path.replace('\\', "/")
}

fn unit_signature(rel_path_posix: &str, suffix: &str) -> String {
    format!("{rel_path_posix}:{suffix}")
}

pub(crate) fn language_for_extension(ext: &str) -> Option<LangPack> {
    match ext.to_ascii_lowercase().as_str() {
        "rs" => Some(LangPack::Rust),
        "py" => Some(LangPack::Python),
        "js" | "jsx" | "mjs" | "cjs" => Some(LangPack::JavaScript),
        "ts" => Some(LangPack::TypeScript),
        "tsx" => Some(LangPack::Tsx),
        _ => None,
    }
}

/// Extract candidate code units within one file based on coarse AST constructs.
///
/// If parsing fails or no units are matched, emits a fallback unit so every indexed file participates.
pub(crate) fn extract_units(rel_path_posix: &str, lang: LangPack, source: &str) -> Vec<CodeUnit> {
    let mut parser = Parser::new();

    match lang {
        // tree-sitter grammar crates expose `LanguageFn`; `Parser::set_language` takes
        // `&Language` built via `&GRAMMAR.into()` (see each grammar crate’s `lib.rs` examples).
        LangPack::Rust => {
            if parser
                .set_language(&tree_sitter_rust::LANGUAGE.into())
                .is_err()
            {
                return fallback(rel_path_posix, source);
            }
        }
        LangPack::Python => {
            if parser
                .set_language(&tree_sitter_python::LANGUAGE.into())
                .is_err()
            {
                return fallback(rel_path_posix, source);
            }
        }
        LangPack::JavaScript => {
            if parser
                .set_language(&tree_sitter_javascript::LANGUAGE.into())
                .is_err()
            {
                return fallback(rel_path_posix, source);
            }
        }
        LangPack::TypeScript => {
            if parser
                .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
                .is_err()
            {
                return fallback(rel_path_posix, source);
            }
        }
        LangPack::Tsx => {
            if parser
                .set_language(&tree_sitter_typescript::LANGUAGE_TSX.into())
                .is_err()
            {
                return fallback(rel_path_posix, source);
            }
        }
    }

    let Some(tree) = parser.parse(source, None) else {
        return fallback(rel_path_posix, source);
    };

    let root = tree.root_node();
    if root.has_error() {
        return fallback(rel_path_posix, source);
    }

    let mut units: Vec<CodeUnit> = Vec::new();
    gather_units(rel_path_posix, source, lang, root, None, "", &mut units);

    if units.is_empty() {
        fallback(rel_path_posix, source)
    } else {
        units
    }
}

fn fallback(rel_path_posix: &str, source: &str) -> Vec<CodeUnit> {
    vec![CodeUnit {
        target_table: "Function",
        target_kind: "Function",
        signature: unit_signature(rel_path_posix, "__file_fallback__"),
        qualified_name: format!("{}::__file_fallback__", posix_rel(rel_path_posix)),
        name_line: 1,
        source_text: source.to_string(),
    }]
}

fn start_line_from_byte(source: &str, start_byte: usize) -> usize {
    source.get(..start_byte).map(|p| p.bytes().filter(|b| *b == b'\n').count() + 1).unwrap_or(1)
}

fn node_text<'a>(source: &'a str, node: tree_sitter::Node<'_>) -> &'a str {
    source
        .get(node.byte_range())
        .unwrap_or("")
}

fn gather_units(
    rel_path_posix: &str,
    source: &str,
    lang: LangPack,
    node: tree_sitter::Node<'_>,
    rust_impl_type: Option<&str>,
    ts_class_stack: &str,
    out: &mut Vec<CodeUnit>,
) {
    let kind = node.kind();

    match lang {
        LangPack::Rust => match kind {
            "function_item" => {
                let name = node
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .unwrap_or("anon");
                let sig = if let Some(t) = rust_impl_type {
                    unit_signature(rel_path_posix, &format!("{t}::{name}"))
                } else {
                    unit_signature(rel_path_posix, name)
                };
                let qn = if let Some(t) = rust_impl_type {
                    format!("{}::{t}::{name}", posix_rel(rel_path_posix))
                } else {
                    format!("{}::{name}", posix_rel(rel_path_posix))
                };
                out.push(CodeUnit {
                    target_table: "Function",
                    target_kind: "Function",
                    signature: sig,
                    qualified_name: qn,
                    name_line: start_line_from_byte(source, node.start_byte()),
                    source_text: node_text(source, node).to_string(),
                });
            }
            "struct_item" | "enum_item" | "trait_item" | "union_item" => {
                let name = node
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .unwrap_or("anon");
                let sig = unit_signature(rel_path_posix, &format!("class:{name}"));
                let qn = format!("{}::{name}", posix_rel(rel_path_posix));
                out.push(CodeUnit {
                    target_table: "Class",
                    target_kind: "Class",
                    signature: sig,
                    qualified_name: qn,
                    name_line: start_line_from_byte(source, node.start_byte()),
                    source_text: node_text(source, node).to_string(),
                });
            }
            "impl_item" => {
                let type_name = node
                    .child_by_field_name("type")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .map(str::trim)
                    .unwrap_or("Impl");
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    gather_units(
                        rel_path_posix,
                        source,
                        lang,
                        child,
                        Some(type_name),
                        ts_class_stack,
                        out,
                    );
                }
                return;
            }
            _ => {}
        },
        LangPack::Python => match kind {
            "function_definition" => {
                let name = node
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .unwrap_or("anon");
                let prefix = ts_class_stack;
                let sig = if prefix.is_empty() {
                    unit_signature(rel_path_posix, name)
                } else {
                    unit_signature(rel_path_posix, &format!("{prefix}.{name}"))
                };
                let qn = if prefix.is_empty() {
                    format!("{}::{name}", posix_rel(rel_path_posix))
                } else {
                    format!("{}::{prefix}.{name}", posix_rel(rel_path_posix))
                };
                out.push(CodeUnit {
                    target_table: "Function",
                    target_kind: "Function",
                    signature: sig,
                    qualified_name: qn,
                    name_line: start_line_from_byte(source, node.start_byte()),
                    source_text: node_text(source, node).to_string(),
                });
            }
            "class_definition" => {
                let name = node
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .unwrap_or("Class");
                let sig = unit_signature(rel_path_posix, &format!("class:{name}"));
                let qn = format!("{}::{name}", posix_rel(rel_path_posix));
                out.push(CodeUnit {
                    target_table: "Class",
                    target_kind: "Class",
                    signature: sig,
                    qualified_name: qn.clone(),
                    name_line: start_line_from_byte(source, node.start_byte()),
                    source_text: node_text(source, node).to_string(),
                });
                let new_stack = if ts_class_stack.is_empty() {
                    name.to_string()
                } else {
                    format!("{ts_class_stack}.{name}")
                };
                let mut cursor = node.walk();
                for child in node.named_children(&mut cursor) {
                    gather_units(rel_path_posix, source, lang, child, None, &new_stack, out);
                }
                return;
            }
            _ => {}
        },
        LangPack::JavaScript | LangPack::TypeScript | LangPack::Tsx => match kind {
            "function_declaration"
            | "generator_function_declaration"
            | "method_definition"
            | "function_expression"
            | "arrow_function"
            | "generator_function"
            => {
                let name_leaf = match lang {
                    LangPack::Rust => unreachable!(),
                    LangPack::Python => unreachable!(),
                    LangPack::JavaScript | LangPack::TypeScript | LangPack::Tsx => {
                        node.child_by_field_name("name")
                    }
                };
                let name = name_leaf
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .unwrap_or("(anonymous)");

                let prefix = ts_class_stack;
                let sig = if prefix.is_empty() {
                    unit_signature(rel_path_posix, name)
                } else {
                    unit_signature(rel_path_posix, &format!("{prefix}.{name}"))
                };
                let qn = if prefix.is_empty() {
                    format!("{}::{name}", posix_rel(rel_path_posix))
                } else {
                    format!("{}::{prefix}.{name}", posix_rel(rel_path_posix))
                };
                out.push(CodeUnit {
                    target_table: "Function",
                    target_kind: "Function",
                    signature: sig,
                    qualified_name: qn,
                    name_line: start_line_from_byte(source, node.start_byte()),
                    source_text: node_text(source, node).to_string(),
                });
            }
            "class_declaration" => {
                let name = node
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .unwrap_or("Class");
                let sig = unit_signature(rel_path_posix, &format!("class:{name}"));
                let qn = format!("{}::{name}", posix_rel(rel_path_posix));
                out.push(CodeUnit {
                    target_table: "Class",
                    target_kind: "Class",
                    signature: sig,
                    qualified_name: qn.clone(),
                    name_line: start_line_from_byte(source, node.start_byte()),
                    source_text: node_text(source, node).to_string(),
                });
                let new_stack = if ts_class_stack.is_empty() {
                    name.to_string()
                } else {
                    format!("{ts_class_stack}.{name}")
                };
                let mut cursor = node.walk();
                for child in node.named_children(&mut cursor) {
                    gather_units(rel_path_posix, source, lang, child, None, &new_stack, out);
                }
                return;
            }
            "interface_declaration" | "enum_declaration" | "type_alias_declaration"
                if matches!(lang, LangPack::TypeScript | LangPack::Tsx) =>
            {
                let name = node
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .unwrap_or("Type");
                let sig = unit_signature(rel_path_posix, &format!("type:{kind}:{name}"));
                let qn = format!("{}::{kind}:{name}", posix_rel(rel_path_posix));
                out.push(CodeUnit {
                    target_table: "Class",
                    target_kind: "Class",
                    signature: sig,
                    qualified_name: qn,
                    name_line: start_line_from_byte(source, node.start_byte()),
                    source_text: node_text(source, node).to_string(),
                });
            }
            _ => {}
        },
    };

    let mut cursor = node.walk();
    let next_impl = rust_impl_type;
    let next_cls = ts_class_stack;
    for child in node.children(&mut cursor) {
        gather_units(rel_path_posix, source, lang, child, next_impl, next_cls, out);
    }
}

pub(crate) fn rel_path_posix(repo_root: &Path, abs_file: &Path) -> Result<String, String> {
    let rel = abs_file.strip_prefix(repo_root).map_err(|_| {
        format!(
            "file {} not under repo root {}",
            abs_file.display(),
            repo_root.display()
        )
    })?;
    Ok(posix_rel(&rel.to_string_lossy()))
}
