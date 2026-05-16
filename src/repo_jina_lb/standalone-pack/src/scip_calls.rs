//! **`CALLS` edges from rust-analyzer `index.scip`.**
//!
//! upstream **rust-analyzer** emits **`SymbolInformation::relationships` empty** (`Vec::new()` in its CLI).
//! When relationships are populated (other SCIP emitters), this crate still consumes them; for **RA**,
//! **`CALLS`** is inferred mainly from **`Document::occurrences`**: definitions anchor symbols to **`(POSIX path,
//! 1-based line)`**, and **non-definition** occurrences become **`caller → callee`** edges using the
//! **nearest preceding callable definition line in that document** plus the occurrence **`symbol`**
//! (lexical heuristic; usages that aren’t **`Function`** nodes in Ladybug are skipped naturally).
//!
//! **Rust-only** (`document.language` starts with **`rust`**).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use protobuf::Enum;
use protobuf::Message;
use scip::symbol;
use scip::types::symbol_information::Kind;
use scip::types::{Document, Index, Occurrence, SymbolInformation, SymbolRole};

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

/// First SCIP **`range`** line (0-based) as **1-based** lines (parity with ingest / [`build_symbol_definition_anchors`]).
fn occurrence_anchor_line_1based(occ: &Occurrence) -> usize {
    let line0 = occ.range.first().copied().unwrap_or(0).max(0) as usize;
    line0.saturating_add(1)
}

/// Callable defs in **`doc`** with global anchors → sorted **(definition line ascending, SCIP symbol id)**.
fn sorted_callable_def_lines(doc: &Document, anchors: &HashMap<String, (String, usize)>) -> Vec<(usize, String)> {
    let mut defs: Vec<(usize, String)> = doc
        .symbols
        .iter()
        .filter(|si| scip_si_is_callable_origin(si))
        .filter_map(|si| anchors.get(&si.symbol).map(|(_, line)| (*line, si.symbol.clone())))
        .collect();
    defs.sort_by(|a, b| a.0.cmp(&b.0));
    defs
}

fn nearest_caller_symbol(sorted_callable_defs: &[(usize, String)], ref_line_1based: usize) -> Option<&str> {
    let idx = sorted_callable_defs.partition_point(|(line, _)| *line <= ref_line_1based);
    if idx == 0 {
        None
    } else {
        Some(sorted_callable_defs[idx - 1].1.as_str())
    }
}

fn try_create_calls_pair(
    conn: &lbug::Connection<'_>,
    registry: &FunctionAnchorRegistry,
    anchors: &HashMap<String, (String, usize)>,
    caller_sym: &str,
    callee_sym: &str,
    written: &mut usize,
    errors: &mut usize,
) {
    let Some((caller_path, caller_line)) = anchors.get(caller_sym) else {
        return;
    };
    let Some((callee_path, callee_line)) = anchors.get(callee_sym) else {
        return;
    };
    let Some(caller_pk) = registry.pk_for_path_line(caller_path, *caller_line) else {
        return;
    };
    let Some(callee_pk) = registry.pk_for_path_line(callee_path, *callee_line) else {
        return;
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

/// Infer **`CALLS`** from occurrences (handles rust-analyzer’s empty **`relationships`**).
fn apply_occurrences_from_doc(
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
    let sorted_defs = sorted_callable_def_lines(doc, anchors);
    if sorted_defs.is_empty() {
        return;
    }
    let def_flag = SymbolRole::Definition.value();
    for occ in &doc.occurrences {
        if occ.symbol.is_empty() || symbol::is_local_symbol(&occ.symbol) {
            continue;
        }
        if (occ.symbol_roles & def_flag) != 0 {
            continue;
        }
        let line_ref = occurrence_anchor_line_1based(occ);
        let Some(caller_sym) = nearest_caller_symbol(&sorted_defs, line_ref) else {
            continue;
        };
        if caller_sym == occ.symbol.as_str() {
            continue;
        }
        try_create_calls_pair(
            conn,
            registry,
            anchors,
            caller_sym,
            occ.symbol.as_str(),
            written,
            errors,
        );
    }
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
        apply_occurrences_from_doc(conn, registry, &anchors, doc, &mut written, &mut errors);
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

    #[test]
    fn nearest_preceding_callable_partition() {
        let defs = [(5_usize, "a".into()), (20_usize, "b".into())];
        assert_eq!(super::nearest_caller_symbol(&defs, 3), None);
        assert_eq!(super::nearest_caller_symbol(&defs, 6), Some("a"));
        assert_eq!(super::nearest_caller_symbol(&defs, 26), Some("b"));
    }
}
