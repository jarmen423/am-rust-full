//! Tiny call graph for SCIP smoke: `caller` → `answer`.

#[inline]
pub fn answer() -> i32 {
    42
}

/// Calls [`answer`] so SCIP should record a callable cross-reference edge.
pub fn caller() -> i32 {
    answer()
}
