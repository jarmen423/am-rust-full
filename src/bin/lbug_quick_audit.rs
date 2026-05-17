//! One-shot Ladybug diagnostics: open a `.lbug` and print canned Cypher result rows (debug output).
//!
//! ```text
//! cargo run -p am-workspace --bin lbug-quick-audit -- "D:\path\file.lbug"
//! ```

#[cfg(not(feature = "ladybug"))]
fn main() {
    eprintln!("Rebuild with `--features ladybug` (enabled by default).");
    std::process::exit(2);
}

#[cfg(feature = "ladybug")]
fn main() -> Result<(), String> {
    use lbug::{Database, SystemConfig, Value};

    fn row_line(row: &[Value]) -> String {
        row.iter().map(|v| v.to_string()).collect::<Vec<_>>().join("\t|\t")
    }

    let path = std::env::args().nth(1).ok_or_else(|| {
        "usage: lbug-quick-audit <PATH.lbug>".to_string()
    })?;

    let db = Database::new(path.as_str(), SystemConfig::default())
        .map_err(|e| format!("Database::new({path:?}): {e}"))?;
    let conn = lbug::Connection::new(&db).map_err(|e| format!("Connection::new: {e}"))?;

    let suite: &[(&str, &str)] = &[
        ("counts_calls", r#"MATCH ()-[r:CALLS]->() RETURN count(r) AS n;"#),
        ("counts_defines_file_fn", r#"MATCH (:File)-[r:DEFINES]->(:Function) RETURN count(r) AS n;"#),
        ("distinct_file_repo_ids_top", r#"MATCH (f:File) RETURN f.repo_id, count(*) ORDER BY count(*) DESC LIMIT 15;"#),
        ("distinct_function_repo_ids_top", r#"MATCH (fn:Function) RETURN fn.repo_id, count(*) ORDER BY count(*) DESC LIMIT 15;"#),
        (
            "sample_functions_agentic_memory_path",
            r#"MATCH (fn:Function) WHERE fn.path CONTAINS 'agentic_memory' RETURN fn.repo_id, fn.path LIMIT 8;"#,
        ),
        (
            "calls_sample_any",
            r#"MATCH (a:Function)-[c:CALLS]->(b:Function) RETURN a.repo_id, a.path, b.path LIMIT 15;"#,
        ),
        (
            "file_defines_calls_full_path_attempt",
            r#"MATCH p = (f1:File)-[:DEFINES]->(caller:Function)-[:CALLS]->(callee:Function)<-[:DEFINES]-(f2:File) RETURN f1.repo_id AS r1, f1.path AS f1p, f2.path AS f2p LIMIT 10;"#,
        ),
    ];

    for (title, cy) in suite {
        eprintln!("\n========== {} ==========", title);
        eprintln!("{}", cy);
        let mut res = conn
            .query(cy)
            .map_err(|e| format!("query failed [{title}]: {e}"))?;
        let names = res.get_column_names();
        eprintln!("columns: {:?}", names);
        let mut nrows = 0u64;
        for row in &mut res {
            nrows += 1;
            println!("{}", row_line(&row));
        }
        eprintln!("(rows printed: {})", nrows);
    }

    Ok(())
}
