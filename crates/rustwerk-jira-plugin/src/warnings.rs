//! Typed, non-fatal warnings produced by the mapping
//! and transition code paths.
//!
//! [`MappingWarning::Display`] owns the wire format so
//! the per-task message stays consistent across emitters
//! (AQ-X5 / AQ-X6 — the previous string-based warnings
//! used an ambiguous `; ` separator that could collide
//! with Jira response bodies).

/// Non-fatal advisory produced while building or
/// transitioning a Jira issue. The caller accumulates
/// a `Vec<MappingWarning>` and renders them once as a
/// `(WARNING: …)` suffix on the task message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MappingWarning {
    /// Configured `assignee_map` had no entry for this
    /// task's assignee; the field was omitted from the
    /// payload.
    UnmappedAssignee(String),
    /// Configured `priority_map` had no entry for this
    /// task's complexity score.
    UnmappedPriority(u32),
    /// A tag was dropped from `fields.labels` because it
    /// contains characters Jira labels reject (whitespace
    /// or control chars).
    RejectedLabel(String),
    /// Jira returned a non-2xx status for the workflow
    /// transition call. The create/update itself
    /// succeeded; the status is out of sync.
    TransitionHttp {
        transition_id: String,
        status: u16,
        body: String,
    },
    /// Transport error during the transition call.
    TransitionTransport {
        transition_id: String,
        message: String,
    },
    /// The parent task's `plugin_state.jira.key` was
    /// present but did not parse as a valid Jira issue
    /// key. The child is pushed without a `parent.key`
    /// field (orphan) rather than failing the whole task.
    InvalidParentKey(String),
}

impl std::fmt::Display for MappingWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnmappedAssignee(email) => write!(
                f,
                "assignee '{email}' has no entry in assignee_map; skipped"
            ),
            Self::UnmappedPriority(c) => write!(
                f,
                "complexity {c} has no entry in priority_map; skipped"
            ),
            Self::RejectedLabel(tag) => write!(
                f,
                "tag '{tag}' contains characters Jira labels reject (whitespace / control); dropped"
            ),
            Self::TransitionHttp {
                transition_id,
                status,
                body,
            } => write!(
                f,
                "transition to {transition_id} returned HTTP {status}: {body}"
            ),
            Self::TransitionTransport {
                transition_id,
                message,
            } => write!(
                f,
                "transition to {transition_id} failed: {message}"
            ),
            Self::InvalidParentKey(raw) => write!(
                f,
                "parent plugin state has invalid Jira issue key '{raw}'; \
                 issue pushed without parent link"
            ),
        }
    }
}
