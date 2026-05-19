//! Bounded read-only Cypher execution for the operator query shell.

use crate::store::ladybug::LadybugDb;
use std::time::Instant;

pub const DEFAULT_ROW_LIMIT: usize = 100;
pub const MAX_QUERY_CHARS: usize = 8_192;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryClassification {
    ReadOnly,
    RejectedMutation,
    Empty,
}

#[derive(Debug, Clone)]
pub struct QueryResult {
    pub classification: QueryClassification,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub truncated: bool,
    pub duration_ms: u64,
}

pub struct LadybugQueryService;

impl LadybugQueryService {
    /// Classify and optionally execute a Cypher string against Ladybug.
    pub fn execute(
        db: Option<&LadybugDb>,
        cypher: &str,
        row_limit: usize,
    ) -> Result<QueryResult, String> {
        let trimmed = cypher.trim();
        if trimmed.is_empty() {
            return Ok(QueryResult {
                classification: QueryClassification::Empty,
                columns: vec![],
                rows: vec![],
                truncated: false,
                duration_ms: 0,
            });
        }
        if trimmed.len() > MAX_QUERY_CHARS {
            return Err("query_limit_exceeded".to_string());
        }
        if !is_readonly_cypher(trimmed) {
            return Ok(QueryResult {
                classification: QueryClassification::RejectedMutation,
                columns: vec![],
                rows: vec![],
                truncated: false,
                duration_ms: 0,
            });
        }
        let Some(db) = db else {
            return Err("ladybug_unavailable".to_string());
        };

        let bounded = ensure_limit(trimmed, row_limit);
        let started = Instant::now();
        let (columns, rows, truncated) =
            crate::store::ladybug::run_adhoc_query(db, &bounded, row_limit)?;
        Ok(QueryResult {
            classification: QueryClassification::ReadOnly,
            columns,
            rows,
            truncated,
            duration_ms: started.elapsed().as_millis() as u64,
        })
    }
}

fn is_readonly_cypher(sql: &str) -> bool {
    let upper = sql.to_ascii_uppercase();
    const BLOCKED: &[&str] = &[
        "CREATE ", "DELETE ", "DETACH ", "DROP ", "MERGE ", "REMOVE ", "SET ", "INSERT ",
    ];
    !BLOCKED.iter().any(|kw| upper.contains(kw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_create() {
        assert!(!is_readonly_cypher("CREATE (n:Foo) RETURN n"));
    }

    #[test]
    fn allows_match() {
        assert!(is_readonly_cypher("MATCH (n) RETURN n LIMIT 5"));
    }
}

fn ensure_limit(sql: &str, cap: usize) -> String {
    let upper = sql.to_ascii_uppercase();
    if upper.contains(" LIMIT ") {
        sql.to_string()
    } else {
        format!("{sql} LIMIT {cap}")
    }
}
