/// Domain errors for the rustwerk project model.
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    /// The referenced task ID does not exist.
    #[error("task not found: {0}")]
    TaskNotFound(String),

    /// A task with this ID already exists.
    #[error("duplicate task ID: {0}")]
    DuplicateTaskId(String),

    /// The requested status transition is not allowed.
    #[error("invalid status transition: {from} -> {to}")]
    InvalidTransition {
        /// Current status.
        from: String,
        /// Requested status.
        to: String,
    },

    /// The effort value is invalid.
    #[error("invalid effort: {0}")]
    InvalidEffort(String),

    /// Adding this dependency would create a cycle.
    #[error("dependency cycle detected: {0}")]
    CycleDetected(String),

    /// A project already exists at this location.
    #[error("project already exists: {0}")]
    ProjectAlreadyExists(String),

    /// The input value failed validation.
    #[error("validation error: {0}")]
    ValidationError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_task_not_found() {
        let err = DomainError::TaskNotFound("T1".into());
        assert_eq!(err.to_string(), "task not found: T1");
    }

    #[test]
    fn error_display_duplicate_task_id() {
        let err = DomainError::DuplicateTaskId("AUTH".into());
        assert_eq!(err.to_string(), "duplicate task ID: AUTH");
    }

    #[test]
    fn error_display_invalid_transition() {
        let err = DomainError::InvalidTransition {
            from: "todo".into(),
            to: "done".into(),
        };
        assert_eq!(
            err.to_string(),
            "invalid status transition: todo -> done"
        );
    }

    #[test]
    fn error_display_invalid_effort() {
        let err = DomainError::InvalidEffort("bad".into());
        assert_eq!(err.to_string(), "invalid effort: bad");
    }

    #[test]
    fn error_display_cycle_detected() {
        let err =
            DomainError::CycleDetected("A -> B -> A".into());
        assert_eq!(
            err.to_string(),
            "dependency cycle detected: A -> B -> A"
        );
    }

    #[test]
    fn error_display_project_already_exists() {
        let err = DomainError::ProjectAlreadyExists(
            ".rustwerk/project.json".into(),
        );
        assert_eq!(
            err.to_string(),
            "project already exists: .rustwerk/project.json"
        );
    }

    #[test]
    fn error_display_validation_error() {
        let err = DomainError::ValidationError(
            "title must not be empty".into(),
        );
        assert_eq!(
            err.to_string(),
            "validation error: title must not be empty"
        );
    }
}
