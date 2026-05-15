//! Internal shim providing the `lbug` crate API surface.
//!
//! This module exposes a minimal subset of the LadybugDB Rust client API
//! (Connection, Statement, Row) so that `ladybug.rs` can compile without the
//! external `lbug` crate being available in all build environments.
//!
//! **When the real `lbug` crate is linked:** replace the `use crate::lbug_shim`
//! imports in `ladybug.rs` with `use lbug` and remove this module.
//!
//! All query methods on the stub return empty results — this provides graceful
//! fallback when no `.lbug` database is present, matching the Phase 4
//! requirement that routes must never 500 even when LadybugDB is offline.

use std::collections::HashMap;

// ── Error type ───────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Error {
    msg: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error {
            msg: format!("io: {}", e),
        }
    }
}

// ── ToSql trait (for query parameters) ───────────────────────────────

pub trait ToSql {
    fn to_sql(&self) -> String;
}

impl ToSql for str {
    fn to_sql(&self) -> String {
        self.to_string()
    }
}

impl ToSql for String {
    fn to_sql(&self) -> String {
        self.clone()
    }
}

impl ToSql for i64 {
    fn to_sql(&self) -> String {
        self.to_string()
    }
}

impl ToSql for i32 {
    fn to_sql(&self) -> String {
        self.to_string()
    }
}

// ── FromSql trait ────────────────────────────────────────────────────

pub trait FromSql: Sized {
    fn from_sql(value: Option<&str>) -> Result<Self, Error>;
}

impl FromSql for String {
    fn from_sql(value: Option<&str>) -> Result<Self, Error> {
        value
            .map(|s| s.to_string())
            .ok_or_else(|| Error {
                msg: "NULL value for String".into(),
            })
    }
}

impl FromSql for i64 {
    fn from_sql(value: Option<&str>) -> Result<Self, Error> {
        value
            .ok_or_else(|| Error {
                msg: "NULL value for i64".into(),
            })?
            .parse()
            .map_err(|e: std::num::ParseIntError| Error {
                msg: format!("parse i64: {}", e),
            })
    }
}

impl FromSql for i32 {
    fn from_sql(value: Option<&str>) -> Result<Self, Error> {
        value
            .ok_or_else(|| Error {
                msg: "NULL value for i32".into(),
            })?
            .parse()
            .map_err(|e: std::num::ParseIntError| Error {
                msg: format!("parse i32: {}", e),
            })
    }
}

impl FromSql for f64 {
    fn from_sql(value: Option<&str>) -> Result<Self, Error> {
        value
            .ok_or_else(|| Error {
                msg: "NULL value for f64".into(),
            })?
            .parse()
            .map_err(|e: std::num::ParseFloatError| Error {
                msg: format!("parse f64: {}", e),
            })
    }
}

impl FromSql for bool {
    fn from_sql(value: Option<&str>) -> Result<Self, Error> {
        match value {
            Some("true") | Some("1") | Some("t") => Ok(true),
            Some("false") | Some("0") | Some("f") => Ok(false),
            _ => Err(Error {
                msg: format!("invalid boolean: {:?}", value),
            }),
        }
    }
}

// ── Connection ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Connection {
    db_path: String,
}

impl Connection {
    /// Open a connection to a `.lbug` database file.
    ///
    /// The shim version checks that the file exists and returns an error
    /// otherwise.  When the real `lbug` crate is used this will load the
    /// shared library and open the native database handle.
    pub fn open(path: &str) -> Result<Self, Error> {
        let p = std::path::Path::new(path);
        if !p.exists() {
            return Err(Error {
                msg: format!("LadybugDB not found at: {}", path),
            });
        }
        Ok(Connection {
            db_path: path.to_string(),
        })
    }

    pub fn path(&self) -> &str {
        &self.db_path
    }

    /// Prepare a Cypher statement for execution.
    ///
    /// The shim stores the SQL/Cypher string; the real implementation would
    /// compile the query into a native execution plan.
    pub fn prepare(&self, sql: &str) -> Result<Statement, Error> {
        Ok(Statement {
            sql: sql.to_string(),
        })
    }
}

// ── Statement ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Statement {
    sql: String,
}

impl Statement {
    /// Execute a Cypher query and map each row through the provided closure.
    ///
    /// **Stub behaviour:** always returns an empty `Vec`.  The real
    /// implementation would iterate native result rows and invoke `f` for
    /// each one.
    pub fn query_map<T, F>(
        &mut self,
        _params: &[&dyn ToSql],
        mut _f: F,
    ) -> Result<Vec<T>, Error>
    where
        F: FnMut(&Row) -> Result<T, Error>,
    {
        // Stub: no rows to iterate
        let _ = self.sql.as_str();
        Ok(Vec::new())
    }
}

// ── Row ──────────────────────────────────────────────────────────────

/// A single result row.
///
/// The shim never produces actual rows (queries always return empty),
/// but the type exists so that closures in `ladybug.rs` compile.
pub struct Row<'a> {
    _values: HashMap<String, Option<String>>,
    _columns: Vec<String>,
    _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> Row<'a> {
    #[allow(dead_code)]
    pub(crate) fn new(columns: Vec<String>, values: Vec<Option<String>>) -> Self {
        let mut map = HashMap::new();
        for (col, val) in columns.iter().zip(values.into_iter()) {
            map.insert(col.clone(), val);
        }
        Row {
            _values: map,
            _columns: columns,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get a column value by its zero-based index.
    pub fn get<T: FromSql>(&self, idx: usize) -> Result<T, Error> {
        let col_name = self
            ._columns
            .get(idx)
            .ok_or_else(|| Error {
                msg: format!("column index {} out of bounds", idx),
            })?;
        let value = self._values.get(col_name.as_str()).and_then(|v| v.as_deref());
        T::from_sql(value)
    }

    /// Get a column value by its name (case-sensitive).
    #[allow(dead_code)]
    pub fn get_by_name<T: FromSql>(&self, name: &str) -> Result<T, Error> {
        let value = self._values.get(name).and_then(|v| v.as_deref());
        T::from_sql(value)
    }
}
