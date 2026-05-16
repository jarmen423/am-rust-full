//! **`CALLS` edges from rust-analyzer `index.scip`.**
//!
//! SCIP encodes cross-symbol edges on [`scip::types::SymbolInformation::relationships`].
//! For **call graph v1** we treat **`Relationship { is_reference: true }`** emanating from a
//! callable symbol (function / method / …) as **`Function → Function` `CALLS`**, then align
//! endpoints to Ladybug rows using **definition line** + path (see [`super::calls_registry`]).
//!
//! This is intentionally **Rust-only**: only `Document.language` values that parse as `rust*`
//! participate. Unresolved pairs are skipped.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use protobuf::Enum;
use protobuf::Message;
use scip::symbol;
use scip::types::symbol_information::Kind;
use scip::types::{Document, Index, SymbolInformation, SymbolRole};

use super::calls_registry::FunctionAnchorRegistry;
use super::ladybug_writes::create_rel_calls;

fn posix_path(s: &str) -> String {
    s.replace('\\', "/")
}

/// `--scip PATH` wins when the file exists; otherwise try a few repo-root layouts.
pub(crate) fn resolve_scip_index_path(repo_root_canon: &Path, explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        if p.is_file() {
            return Some(p.to_path_buf());
        }
    }

    [
        repo_root_canon.join("index.scip"),
        repo_root_canon.join("target").join("index.scip"),
        repo_root_canon.join(".scip").join("index.scip"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

fn document_is_rust(doc: &Document) -> bool {
    doc.language.to_ascii_lowercase().starts_with("rust")
}

/// True when this symbol is plausible as a **caller** (body that can hold references).
fn scip_si_is_callable_origin(si: &SymbolInformation) -> bool {
    let k = si.kind.enum_value_or(Kind::UnspecifiedKind);
    matches!(
        k,
        Kind::Function
            | Kind::Method
            | Kind::TraitMethod
            | Kind::StaticMethod
            | Kind::Macro
            | Kind::Accessor
            | Kind::Getter
            | Kind::Setter
            | Kind::AbstractMethod
            | Kind::Constructor
    )
}

fn build_symbol_definition_anchors(index: &Index) -> HashMap<String, (String, usize)> {
    let def_flag = SymbolRole::Definition.value();
    let mut map: HashMap<String, (String, usize)> = HashMap::new();

    for doc in &index.documents {
        if !document_is_rust(doc) {
            continue;
        }
        let path = posix_path(&doc.relative_path);
        for occ in &doc.occurrences {
            if occ.symbol.is_empty() || symbol::is_local_symbol(&occ.symbol) {
                continue;
            }
            if (occ.symbol_roles & def_flag) == 0 {
                continue;
            }
            let line0 = occ.range.first().copied().unwrap_or(0).max(0) as usize;
            let line_1based = line0.saturating_add(1);
            map.insert(occ.symbol.clone(), (path.clone(), line_1based));
        }
    }

    map
}

fn apply_relationships_from_doc(
    conn: &lbug::Connection<'_>,
    registry: &FunctionAnchorRegistry,
    anchors: &HashMap<String, (String, usize)>,
    doc: &Document,
    written: &mut usize,
    errors: &mut usize,
) {
    if !document_is_rust(doc) {
        return;
    }

    for si in &doc.symbols {
        if !scip_si_is_callable_origin(si) {
            continue;
        }

        let Some((caller_path, caller_line)) = anchors.get(&si.symbol) else {
            continue;
        };
        let Some(caller_pk) = registry.pk_for_path_line(caller_path, *caller_line) else {
            continue;
        };

        for rel in &si.relationships {
            if !rel.is_reference || rel.is_type_definition {
                continue;
            }
            if rel.symbol.is_empty() || symbol::is_local_symbol(&rel.symbol) {
                continue;
            }

            let Some((callee_path, callee_line)) = anchors.get(&rel.symbol) else {
                continue;
            };
            let Some(callee_pk) = registry.pk_for_path_line(callee_path, *callee_line) else {
                continue;
            };

            match create_rel_calls(conn, caller_pk, callee_pk) {
                Ok(true) => *written += 1,
                Ok(false) => {}
                Err(e) => {
                    *errors += 1;
                    eprintln!("scip: CALLS {:?} -> {:?}: {e}", caller_pk, callee_pk);
                }
            }
        }
    }
}

/// Parse `index.scip` and write **`CALLS`** rows. Returns `(edges_written, anchors_in_index)`.
///
/// `repo_id` is reserved for future filtering; all anchors must already match the ingest run.
pub(crate) fn ingest_scip_calls(
    conn: &lbug::Connection<'_>,
    registry: &FunctionAnchorRegistry,
    scip_path: &Path,
    _repo_id: &str,
) -> Result<(usize, usize), String> {
    let bytes = std::fs::read(scip_path).map_err(|e| format!("read {}: {e}", scip_path.display()))?;
    let index = Index::parse_from_bytes(&bytes).map_err(|e| format!("parse SCIP {}: {e}", scip_path.display()))?;

    let anchors = build_symbol_definition_anchors(&index);
    let anchor_count = anchors.len();

    let mut written = 0usize;
    let mut errors = 0usize;

    for doc in &index.documents {
        apply_relationships_from_doc(conn, registry, &anchors, doc, &mut written, &mut errors);
    }

    // `external_symbols` can hold package metadata; Rust call targets in the same workspace
    // are already covered via `documents[].symbols`.

    if errors > 0 {
        eprintln!("scip: {errors} CALLS edge errors writing from {}", scip_path.display());
    }

    Ok((written, anchor_count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use scip::types::{Document, Occurrence};

    #[test]
    fn build_anchors_from_synthetic_index() {
        let mut idx = Index::new();
        let mut doc = Document::new();
        doc.language = "rust".into();
        doc.relative_path = "src/lib.rs".into();
        let mut occ = Occurrence::new();
        occ.symbol = "testSymbolRust".into();
        occ.range = vec![9i32, 0, 9, 5];
        occ.symbol_roles = SymbolRole::Definition.value();
        doc.occurrences.push(occ);
        idx.documents.push(doc);

        let m = build_symbol_definition_anchors(&idx);
        assert_eq!(
            m.get("testSymbolRust"),
            Some(&("src/lib.rs".into(), 10usize)),
            "SCIP lines are 0-based; anchors use 1-based name_line parity with tree-sitter"
        );
    }
}
