//! **`(repo-relative path, 1-based source line)` → `Function` pk** map,
//! filled during the tree-sitter / Jina walk and read by the rust-analyzer SCIP
//! pass when materializing `CALLS` edges.

use std::collections::HashMap;

/// Keys use POSIX path segments (mirrors [`super::parse::rel_path_posix`]).
#[derive(Default, Debug)]
pub(crate) struct FunctionAnchorRegistry {
    by_path_and_line: HashMap<(String, usize), String>,
}

impl FunctionAnchorRegistry {
    /// Record one **Function** node's primary key for SCIP line-anchor resolution.
    ///
    /// `name_line_1based` matches [`super::parse::CodeUnit::name_line`] (1-based;
    /// SCIP occurrence lines are converted to the same convention when ingesting).
    pub(crate) fn record_function_anchor(
        &mut self,
        rel_path_posix: &str,
        name_line_1based: usize,
        function_pk: String,
    ) {
        let path = rel_path_posix.replace('\\', "/");
        self.by_path_and_line
            .insert((path, name_line_1based), function_pk);
    }

    pub(crate) fn pk_for_path_line(
        &self,
        rel_path_posix: &str,
        name_line_1based: usize,
    ) -> Option<&str> {
        let path = rel_path_posix.replace('\\', "/");
        self.by_path_and_line
            .get(&(path, name_line_1based))
            .map(|s| s.as_str())
    }

    pub(crate) fn len(&self) -> usize {
        self.by_path_and_line.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_backslashes_in_path_key() {
        let mut r = FunctionAnchorRegistry::default();
        r.record_function_anchor("src\\foo\\bar.rs", 10, "function:x".into());
        assert_eq!(r.pk_for_path_line("src/foo/bar.rs", 10), Some("function:x"));
    }
}
