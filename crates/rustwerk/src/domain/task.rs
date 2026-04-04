use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::error::DomainError;

/// Unique identifier for a task — either user-supplied
/// mnemonic or auto-generated.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct TaskId(String);

impl TaskId {
    /// Create a new task ID from a string. Must be
    /// non-empty and contain only alphanumeric characters,
    /// hyphens, and underscores.
    pub fn new(id: &str) -> Result<Self, DomainError> {
        if id.is_empty() {
            return Err(DomainError::ValidationError(
                "task ID must not be empty".into(),
            ));
        }
        if !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(DomainError::ValidationError(format!(
                "task ID must contain only alphanumeric \
                     characters, hyphens, and underscores: \
                     {id}"
            )));
        }
        Ok(Self(id.to_uppercase()))
    }

    /// Return the ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Generate an auto-ID from a sequence number.
    /// Zero-padded to 4 digits for correct sort order.
    pub fn auto(n: u32) -> Self {
        Self(format!("T{n:04}"))
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Task status in the workflow.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Not yet started.
    #[default]
    Todo,
    /// Currently being worked on.
    InProgress,
    /// Waiting on dependencies or external input.
    Blocked,
    /// Completed.
    Done,
    /// Intentionally deferred.
    OnHold,
}

impl Status {
    /// Check whether a transition to the target status
    /// is allowed.
    pub fn can_transition_to(self, target: Self) -> bool {
        matches!(
            (self, target),
            (Self::Todo | Self::Blocked | Self::OnHold, Self::InProgress,)
                | (Self::Todo | Self::InProgress, Self::OnHold,)
                | (Self::InProgress, Self::Done | Self::Blocked)
                | (Self::Blocked | Self::OnHold, Self::Todo)
        )
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Todo => write!(f, "TODO"),
            Self::InProgress => write!(f, "IN_PROGRESS"),
            Self::Blocked => write!(f, "BLOCKED"),
            Self::Done => write!(f, "DONE"),
            Self::OnHold => write!(f, "ON_HOLD"),
        }
    }
}

/// Time unit for effort values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffortUnit {
    /// Hours.
    H,
    /// Days.
    D,
    /// Weeks.
    W,
    /// Months.
    M,
}

impl fmt::Display for EffortUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::H => write!(f, "H"),
            Self::D => write!(f, "D"),
            Self::W => write!(f, "W"),
            Self::M => write!(f, "M"),
        }
    }
}

/// An effort value with a numeric amount and time unit.
/// Serializes as a string like `"2.5H"`.
#[derive(Debug, Clone, PartialEq)]
pub struct Effort {
    /// Numeric amount (must be positive).
    pub value: f64,
    /// Time unit.
    pub unit: EffortUnit,
}

impl Effort {
    /// Parse an effort string like `"2.5H"`, `"1D"`,
    /// `"0.5W"`, `"1M"`.
    pub fn parse(s: &str) -> Result<Self, DomainError> {
        let s = s.trim();
        if s.is_empty() {
            return Err(DomainError::InvalidEffort(
                "effort string must not be empty".into(),
            ));
        }

        let last_char = s.chars().last().unwrap_or(' ');
        let num_str = &s[..s.len() - last_char.len_utf8()];
        let unit = match last_char.to_uppercase().next().unwrap_or(' ') {
            'H' => EffortUnit::H,
            'D' => EffortUnit::D,
            'W' => EffortUnit::W,
            'M' => EffortUnit::M,
            _ => {
                return Err(DomainError::InvalidEffort(format!(
                    "unknown effort unit: {last_char} \
                         (expected H, D, W, or M)"
                )))
            }
        };

        let value: f64 = num_str.parse().map_err(|_| {
            DomainError::InvalidEffort(format!(
                "invalid effort number: {num_str}"
            ))
        })?;

        if !value.is_finite() || value <= 0.0 {
            return Err(DomainError::InvalidEffort(
                "effort must be a finite positive number".into(),
            ));
        }

        Ok(Self { value, unit })
    }

    /// Convert to hours using standard conversions:
    /// 1D = 8H, 1W = 40H, 1M = 160H.
    pub fn to_hours(&self) -> f64 {
        match self.unit {
            EffortUnit::H => self.value,
            EffortUnit::D => self.value * 8.0,
            EffortUnit::W => self.value * 40.0,
            EffortUnit::M => self.value * 160.0,
        }
    }
}

impl fmt::Display for Effort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Avoid trailing ".0" for whole numbers.
        if self.value.fract() == 0.0 {
            write!(f, "{:.0}{}", self.value, self.unit)
        } else {
            write!(f, "{}{}", self.value, self.unit)
        }
    }
}

impl Serialize for Effort {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Effort {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).map_err(serde::de::Error::custom)
    }
}

/// A logged effort entry for a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffortEntry {
    /// Amount of effort spent.
    pub effort: Effort,
    /// Who did the work.
    pub developer: String,
    /// When the effort was logged.
    pub timestamp: DateTime<Utc>,
    /// Optional note about what was done.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// A validated, normalized tag for task categorization.
///
/// Tags are slug-like: lowercase ASCII alphanumeric with
/// hyphens, max 50 characters. Stored sorted and
/// deduplicated within a task.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub struct Tag(String);

impl Tag {
    /// Maximum length of a tag in characters.
    pub const MAX_LEN: usize = 50;

    /// Create a new tag. Input is trimmed and lowercased.
    /// Must be non-empty, at most 50 characters, and
    /// contain only ASCII alphanumeric characters and
    /// hyphens.
    pub fn new(tag: &str) -> Result<Self, DomainError> {
        let tag = tag.trim().to_lowercase();
        if tag.is_empty() {
            return Err(DomainError::ValidationError(
                "tag must not be empty".into(),
            ));
        }
        if tag.len() > Self::MAX_LEN {
            return Err(DomainError::ValidationError(
                format!(
                    "tag must be at most {} characters \
                     (got {})",
                    Self::MAX_LEN,
                    tag.len()
                ),
            ));
        }
        if !tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err(DomainError::ValidationError(
                format!(
                    "tag must contain only lowercase \
                     alphanumeric characters and hyphens: \
                     {tag}"
                ),
            ));
        }
        Ok(Self(tag))
    }

    /// Return the tag as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Serialize for Tag {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Tag {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::new(&s).map_err(serde::de::Error::custom)
    }
}

/// A task in the work breakdown structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Human-readable title.
    pub title: String,
    /// Optional longer description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Current status.
    #[serde(default)]
    pub status: Status,
    /// IDs of tasks this task depends on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<TaskId>,
    /// Estimated effort to complete.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort_estimate: Option<Effort>,
    /// Complexity score (e.g. Fibonacci).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complexity: Option<u32>,
    /// Developer assigned to this task.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
    /// Logged effort entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effort_entries: Vec<EffortEntry>,
    /// Optional tags for categorization and filtering.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<Tag>,
}

impl Task {
    /// Maximum allowed complexity value.
    pub const MAX_COMPLEXITY: u32 = 1000;

    /// Set complexity, validating the range (1..=1000).
    pub fn set_complexity(&mut self, value: u32) -> Result<(), DomainError> {
        if value == 0 || value > Self::MAX_COMPLEXITY {
            return Err(DomainError::ValidationError(format!(
                "complexity must be between 1 and {} \
                     (got {value})",
                Self::MAX_COMPLEXITY
            )));
        }
        self.complexity = Some(value);
        Ok(())
    }

    /// Maximum number of tags per task.
    pub const MAX_TAGS: usize = 20;

    /// Add a tag to this task. Returns `Ok(true)` if the
    /// tag was added, `Ok(false)` if it was already present.
    pub fn add_tag(
        &mut self,
        tag: &str,
    ) -> Result<bool, DomainError> {
        let tag = Tag::new(tag)?;
        match self.tags.binary_search(&tag) {
            Ok(_) => Ok(false),
            Err(pos) => {
                if self.tags.len() >= Self::MAX_TAGS {
                    return Err(DomainError::ValidationError(
                        format!(
                            "a task may have at most {} tags",
                            Self::MAX_TAGS
                        ),
                    ));
                }
                self.tags.insert(pos, tag);
                Ok(true)
            }
        }
    }

    /// Remove a tag from this task. Returns `Ok(true)` if
    /// the tag was removed, `Ok(false)` if not present.
    pub fn remove_tag(
        &mut self,
        tag: &str,
    ) -> Result<bool, DomainError> {
        let tag = Tag::new(tag)?;
        match self.tags.binary_search(&tag) {
            Ok(pos) => {
                self.tags.remove(pos);
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }

    /// Check whether this task has a given tag
    /// (case-insensitive).
    pub fn has_tag(&self, tag: &str) -> bool {
        Tag::new(tag).is_ok_and(|t| {
            self.tags.binary_search(&t).is_ok()
        })
    }

    /// Total logged effort in hours across all entries.
    pub fn total_actual_effort_hours(&self) -> f64 {
        self.effort_entries
            .iter()
            .map(|e| e.effort.to_hours())
            .sum()
    }

    /// Create a new task with the given title.
    pub fn new(title: &str) -> Result<Self, DomainError> {
        let title = title.trim();
        if title.is_empty() {
            return Err(DomainError::ValidationError(
                "task title must not be empty".into(),
            ));
        }
        Ok(Self {
            title: title.to_string(),
            description: None,
            status: Status::default(),
            dependencies: Vec::new(),
            effort_estimate: None,
            complexity: None,
            assignee: None,
            effort_entries: Vec::new(),
            tags: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_id_valid() {
        let id = TaskId::new("AUTH-LOGIN").unwrap();
        assert_eq!(id.as_str(), "AUTH-LOGIN");
    }

    #[test]
    fn task_id_uppercased() {
        let id = TaskId::new("auth-login").unwrap();
        assert_eq!(id.as_str(), "AUTH-LOGIN");
    }

    #[test]
    fn task_id_empty_rejected() {
        assert!(TaskId::new("").is_err());
    }

    #[test]
    fn task_id_invalid_chars_rejected() {
        assert!(TaskId::new("auth login").is_err());
        assert!(TaskId::new("auth.login").is_err());
    }

    #[test]
    fn task_id_auto_generation() {
        let id = TaskId::auto(1);
        assert_eq!(id.as_str(), "T0001");
        let id = TaskId::auto(42);
        assert_eq!(id.as_str(), "T0042");
    }

    #[test]
    fn status_display() {
        assert_eq!(Status::Todo.to_string(), "TODO");
        assert_eq!(Status::InProgress.to_string(), "IN_PROGRESS");
        assert_eq!(Status::Blocked.to_string(), "BLOCKED");
        assert_eq!(Status::Done.to_string(), "DONE");
    }

    #[test]
    fn status_default_is_todo() {
        assert_eq!(Status::default(), Status::Todo);
    }

    #[test]
    fn status_valid_transitions() {
        assert!(Status::Todo.can_transition_to(Status::InProgress));
        assert!(Status::InProgress.can_transition_to(Status::Done));
        assert!(Status::InProgress.can_transition_to(Status::Blocked));
        assert!(Status::Blocked.can_transition_to(Status::InProgress));
        assert!(Status::Blocked.can_transition_to(Status::Todo));
        // On-hold transitions.
        assert!(Status::Todo.can_transition_to(Status::OnHold));
        assert!(Status::OnHold.can_transition_to(Status::Todo));
        assert!(Status::InProgress.can_transition_to(Status::OnHold));
        assert!(Status::OnHold.can_transition_to(Status::InProgress));
    }

    #[test]
    fn status_invalid_transitions() {
        assert!(!Status::Todo.can_transition_to(Status::Done));
        assert!(!Status::Todo.can_transition_to(Status::Blocked));
        assert!(!Status::Done.can_transition_to(Status::Todo));
        assert!(!Status::Done.can_transition_to(Status::InProgress));
        assert!(!Status::OnHold.can_transition_to(Status::Done));
        assert!(!Status::Done.can_transition_to(Status::OnHold));
    }

    #[test]
    fn status_display_on_hold() {
        assert_eq!(Status::OnHold.to_string(), "ON_HOLD");
    }

    #[test]
    fn effort_parse_hours() {
        let e = Effort::parse("2.5H").unwrap();
        assert!((e.value - 2.5).abs() < f64::EPSILON);
        assert_eq!(e.unit, EffortUnit::H);
    }

    #[test]
    fn effort_parse_days() {
        let e = Effort::parse("1D").unwrap();
        assert!((e.value - 1.0).abs() < f64::EPSILON);
        assert_eq!(e.unit, EffortUnit::D);
    }

    #[test]
    fn effort_parse_weeks() {
        let e = Effort::parse("0.5W").unwrap();
        assert!((e.value - 0.5).abs() < f64::EPSILON);
        assert_eq!(e.unit, EffortUnit::W);
    }

    #[test]
    fn effort_parse_months() {
        let e = Effort::parse("3M").unwrap();
        assert!((e.value - 3.0).abs() < f64::EPSILON);
        assert_eq!(e.unit, EffortUnit::M);
    }

    #[test]
    fn effort_parse_case_insensitive() {
        let e = Effort::parse("2h").unwrap();
        assert_eq!(e.unit, EffortUnit::H);
    }

    #[test]
    fn effort_parse_zero_rejected() {
        assert!(Effort::parse("0H").is_err());
    }

    #[test]
    fn effort_parse_negative_rejected() {
        assert!(Effort::parse("-1H").is_err());
    }

    #[test]
    fn effort_parse_empty_rejected() {
        assert!(Effort::parse("").is_err());
    }

    #[test]
    fn effort_parse_unknown_unit_rejected() {
        assert!(Effort::parse("5X").is_err());
    }

    #[test]
    fn effort_parse_infinity_rejected() {
        assert!(Effort::parse("infH").is_err());
    }

    #[test]
    fn effort_parse_nan_rejected() {
        assert!(Effort::parse("NaNH").is_err());
    }

    #[test]
    fn effort_display_whole_number() {
        let e = Effort {
            value: 5.0,
            unit: EffortUnit::H,
        };
        assert_eq!(e.to_string(), "5H");
    }

    #[test]
    fn effort_display_fractional() {
        let e = Effort {
            value: 2.5,
            unit: EffortUnit::D,
        };
        assert_eq!(e.to_string(), "2.5D");
    }

    #[test]
    fn task_new_valid() {
        let t = Task::new("Implement login").unwrap();
        assert_eq!(t.title, "Implement login");
        assert_eq!(t.status, Status::Todo);
        assert!(t.dependencies.is_empty());
        assert!(t.effort_entries.is_empty());
        assert!(t.tags.is_empty());
    }

    // --- Tag tests ---

    #[test]
    fn tag_valid() {
        let t = Tag::new("backend").unwrap();
        assert_eq!(t.as_str(), "backend");
    }

    #[test]
    fn tag_trims_and_lowercases() {
        let t = Tag::new("  Backend  ").unwrap();
        assert_eq!(t.as_str(), "backend");
    }

    #[test]
    fn tag_with_hyphens() {
        let t = Tag::new("phase-1").unwrap();
        assert_eq!(t.as_str(), "phase-1");
    }

    #[test]
    fn tag_empty_rejected() {
        assert!(Tag::new("").is_err());
        assert!(Tag::new("   ").is_err());
    }

    #[test]
    fn tag_spaces_rejected() {
        assert!(Tag::new("hello world").is_err());
    }

    #[test]
    fn tag_special_chars_rejected() {
        assert!(Tag::new("a.b").is_err());
        assert!(Tag::new("a_b").is_err());
        assert!(Tag::new("a/b").is_err());
    }

    #[test]
    fn tag_too_long_rejected() {
        let long = "a".repeat(51);
        assert!(Tag::new(&long).is_err());
        // Exactly 50 is fine.
        let ok = "a".repeat(50);
        assert!(Tag::new(&ok).is_ok());
    }

    #[test]
    fn tag_display() {
        let t = Tag::new("backend").unwrap();
        assert_eq!(t.to_string(), "backend");
    }

    #[test]
    fn task_add_tag() {
        let mut t = Task::new("Test").unwrap();
        assert_eq!(t.add_tag("backend").unwrap(), true);
        assert_eq!(
            t.tags,
            vec![Tag::new("backend").unwrap()]
        );
    }

    #[test]
    fn task_add_tag_duplicate_returns_false() {
        let mut t = Task::new("Test").unwrap();
        assert_eq!(t.add_tag("backend").unwrap(), true);
        assert_eq!(t.add_tag("backend").unwrap(), false);
        assert_eq!(t.tags.len(), 1);
    }

    #[test]
    fn task_add_tag_max_limit() {
        let mut t = Task::new("Test").unwrap();
        for i in 0..Task::MAX_TAGS {
            t.add_tag(&format!("tag-{i:02}")).unwrap();
        }
        assert!(t.add_tag("one-too-many").is_err());
    }

    #[test]
    fn task_remove_tag() {
        let mut t = Task::new("Test").unwrap();
        t.add_tag("backend").unwrap();
        t.add_tag("urgent").unwrap();
        assert_eq!(t.remove_tag("backend").unwrap(), true);
        assert_eq!(
            t.tags,
            vec![Tag::new("urgent").unwrap()]
        );
    }

    #[test]
    fn task_remove_tag_not_found() {
        let mut t = Task::new("Test").unwrap();
        assert_eq!(t.remove_tag("nope").unwrap(), false);
    }

    #[test]
    fn task_has_tag() {
        let mut t = Task::new("Test").unwrap();
        t.add_tag("backend").unwrap();
        assert!(t.has_tag("backend"));
        assert!(t.has_tag("Backend"));
        assert!(!t.has_tag("frontend"));
    }

    #[test]
    fn task_has_tag_invalid_returns_false() {
        let t = Task::new("Test").unwrap();
        assert!(!t.has_tag("not valid!"));
    }

    #[test]
    fn task_tags_sorted() {
        let mut t = Task::new("Test").unwrap();
        t.add_tag("zebra").unwrap();
        t.add_tag("alpha").unwrap();
        t.add_tag("middle").unwrap();
        let names: Vec<&str> =
            t.tags.iter().map(Tag::as_str).collect();
        assert_eq!(names, vec!["alpha", "middle", "zebra"]);
    }

    #[test]
    fn task_tags_serialization_round_trip() {
        let mut t = Task::new("Test").unwrap();
        t.add_tag("backend").unwrap();
        t.add_tag("urgent").unwrap();

        let json = serde_json::to_string(&t).unwrap();
        let t2: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(t2.tags, t.tags);
    }

    #[test]
    fn task_tags_omitted_when_empty() {
        let t = Task::new("Test").unwrap();
        let json = serde_json::to_string(&t).unwrap();
        assert!(!json.contains("tags"));
    }

    #[test]
    fn task_tags_deserialized_from_missing_field() {
        let json = r#"{"title": "Test"}"#;
        let t: Task = serde_json::from_str(json).unwrap();
        assert!(t.tags.is_empty());
    }

    #[test]
    fn task_tags_invalid_json_rejected() {
        let json =
            r#"{"title": "Test", "tags": ["has spaces"]}"#;
        assert!(serde_json::from_str::<Task>(json).is_err());
    }

    #[test]
    fn task_tags_deserialized_normalized() {
        let json =
            r#"{"title": "Test", "tags": ["backend"]}"#;
        let t: Task = serde_json::from_str(json).unwrap();
        assert_eq!(
            t.tags,
            vec![Tag::new("backend").unwrap()]
        );
    }

    #[test]
    fn task_new_trims_whitespace() {
        let t = Task::new("  hello  ").unwrap();
        assert_eq!(t.title, "hello");
    }

    #[test]
    fn task_new_empty_rejected() {
        assert!(Task::new("").is_err());
        assert!(Task::new("   ").is_err());
    }
}
