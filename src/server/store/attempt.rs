//! In-memory workflow attempt store (process-local).

use am_workspace::model::diagnostics::{WorkflowAttempt, WorkflowState, WorkflowStepEvent};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Clone, Default, Debug)]
pub struct AttemptStore {
    inner: Arc<RwLock<HashMap<String, WorkflowAttempt>>>,
}

impl AttemptStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, attempt: WorkflowAttempt) {
        if let Ok(mut guard) = self.inner.write() {
            guard.insert(attempt.attempt_id.clone(), attempt);
        }
    }

    pub fn get(&self, attempt_id: &str) -> Option<WorkflowAttempt> {
        self.inner.read().ok()?.get(attempt_id).cloned()
    }

    pub fn len(&self) -> usize {
        self.inner.read().map(|g| g.len()).unwrap_or(0)
    }

    pub fn update<F>(&self, attempt_id: &str, f: F) -> Option<WorkflowAttempt>
    where
        F: FnOnce(&mut WorkflowAttempt),
    {
        let mut guard = self.inner.write().ok()?;
        let attempt = guard.get_mut(attempt_id)?;
        f(attempt);
        attempt.updated_at = Utc::now();
        Some(attempt.clone())
    }

    pub fn push_step(&self, attempt_id: &str, step: WorkflowStepEvent) {
        let _ = self.update(attempt_id, |a| a.steps.push(step));
    }

    pub fn set_state(&self, attempt_id: &str, state: WorkflowState) {
        let _ = self.update(attempt_id, |a| a.state = state);
    }

    pub fn fail(
        &self,
        attempt_id: &str,
        error_code: &str,
        user_message: &str,
    ) -> Option<WorkflowAttempt> {
        self.update(attempt_id, |a| {
            a.state = WorkflowState::Failed;
            a.error_code = Some(error_code.to_string());
            a.user_message = Some(user_message.to_string());
        })
    }
}
