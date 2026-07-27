//! Intent parsing and generation logic.
//!
//! This module provides pure functions for parsing and generating intent spec files.
//! All functions are deterministic and have no side effects.

use crate::core::models::intent::{Criterion, Intent, IntentStatus, SpecDocument, SpecFrontmatter};
use std::collections::HashMap;
use std::path::PathBuf;

/// Error type for intent parsing operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IntentParseError {
    /// Invalid frontmatter YAML
    InvalidFrontmatter(String),
    /// Missing required field
    MissingField(String),
    /// Invalid markdown structure
    InvalidMarkdown(String),
}

impl std::fmt::Display for IntentParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntentParseError::InvalidFrontmatter(msg) => {
                write!(f, "Invalid frontmatter: {}", msg)
            }
            IntentParseError::MissingField(field) => write!(f, "Missing required field: {}", field),
            IntentParseError::InvalidMarkdown(msg) => write!(f, "Invalid markdown: {}", msg),
        }
    }
}

impl std::error::Error for IntentParseError {}

/// Parse a spec document from raw content.
///
/// The expected format is:
/// ```markdown
/// ---
/// yaml: frontmatter
/// ---
///
/// # Title
///
/// Content...
/// ```
///
/// # Examples
///
/// ```
/// use rightclick::core::logic::intent::parse_spec_document;
///
/// let content = r#"---
/// id: test-123
/// status: draft
/// ---
///
/// # Test Intent
///
/// Description here.
/// "#;
///
/// let doc = parse_spec_document(content).unwrap();
/// assert_eq!(doc.frontmatter.id, Some("test-123".to_string()));
/// assert!(doc.content.contains("Test Intent"));
/// ```
pub fn parse_spec_document(content: &str) -> Result<SpecDocument, IntentParseError> {
    // Check for frontmatter delimiters
    if !content.starts_with("---") {
        // No frontmatter, treat entire content as markdown
        return Ok(SpecDocument {
            frontmatter: SpecFrontmatter::default(),
            content: content.to_string(),
        });
    }

    // Find the end of frontmatter
    let end_marker = content[3..].find("---").ok_or_else(|| {
        IntentParseError::InvalidFrontmatter("Missing closing frontmatter delimiter".to_string())
    })?;

    let frontmatter_str = &content[3..3 + end_marker].trim();
    let content_str = content[3 + end_marker + 3..].trim_start();

    // Parse YAML frontmatter
    let frontmatter: SpecFrontmatter = serde_yaml::from_str(frontmatter_str)
        .map_err(|e| IntentParseError::InvalidFrontmatter(e.to_string()))?;

    Ok(SpecDocument {
        frontmatter,
        content: content_str.to_string(),
    })
}

/// Generate a spec document from components.
///
/// # Examples
///
/// ```
/// use rightclick::core::logic::intent::{generate_spec_document, parse_spec_document};
/// use rightclick::core::models::intent::{SpecFrontmatter, IntentStatus};
///
/// let frontmatter = SpecFrontmatter {
///     id: Some("test-123".to_string()),
///     status: Some(IntentStatus::Draft),
///     ..Default::default()
/// };
///
/// let content = "# Test\n\nDescription.";
/// let doc = generate_spec_document(&frontmatter, content);
///
/// assert!(doc.contains("---"));
/// assert!(doc.contains("id: test-123"));
/// assert!(doc.contains("# Test"));
/// ```
pub fn generate_spec_document(frontmatter: &SpecFrontmatter, content: &str) -> String {
    let yaml = serde_yaml::to_string(frontmatter).unwrap_or_default();

    format!("---\n{}---\n\n{}", yaml, content)
}

/// Extract the title from markdown content.
///
/// Looks for the first H1 heading (# Title).
///
/// # Examples
///
/// ```
/// use rightclick::core::logic::intent::extract_title;
///
/// let content = "# My Intent Title\n\nSome description.";
/// assert_eq!(extract_title(content), Some("My Intent Title".to_string()));
///
/// let no_title = "Just content without heading.";
/// assert_eq!(extract_title(no_title), None);
/// ```
pub fn extract_title(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(title) = trimmed.strip_prefix("# ") {
            return Some(title.trim().to_string());
        }
    }
    None
}

/// Extract description from markdown content.
///
/// Returns the first paragraph after the title.
///
/// # Examples
///
/// ```
/// use rightclick::core::logic::intent::extract_description;
///
/// let content = "# Title\n\nThis is the description.\n\nMore content.";
/// assert_eq!(extract_description(content), Some("This is the description.".to_string()));
/// ```
pub fn extract_description(content: &str) -> Option<String> {
    let mut found_title = false;
    let mut description = String::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("# ") {
            found_title = true;
            continue;
        }

        if found_title {
            if trimmed.is_empty() && !description.is_empty() {
                // End of first paragraph
                break;
            }
            if !trimmed.is_empty() {
                if !description.is_empty() {
                    description.push(' ');
                }
                description.push_str(trimmed);
            }
        }
    }

    if description.is_empty() {
        None
    } else {
        Some(description)
    }
}

/// Extract acceptance criteria from markdown content.
///
/// Looks for a section titled "Acceptance Criteria", "Criteria", or similar,
/// then parses checklist items (- [ ] or - [x]).
///
/// # Examples
///
/// ```
/// use rightclick::core::logic::intent::extract_acceptance_criteria;
///
/// let content = r#"
/// # Title
///
/// ## Acceptance Criteria
///
/// - [ ] First criterion
/// - [x] Second criterion (done)
/// - [ ] Third criterion
/// "#;
///
/// let criteria = extract_acceptance_criteria(content);
/// assert_eq!(criteria.len(), 3);
/// assert!(!criteria[0].completed);
/// assert!(criteria[1].completed);
/// assert!(!criteria[2].completed);
/// ```
pub fn extract_acceptance_criteria(content: &str) -> Vec<Criterion> {
    let mut criteria = Vec::new();
    let mut in_criteria_section = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Check for criteria section header
        if trimmed.to_lowercase().contains("acceptance criteria")
            || trimmed.to_lowercase().contains("## criteria")
        {
            in_criteria_section = true;
            continue;
        }

        // Exit criteria section on next header
        if in_criteria_section && trimmed.starts_with("## ") {
            break;
        }

        // Parse checklist items
        if in_criteria_section {
            if let Some(criterion) = parse_criterion_line(trimmed) {
                criteria.push(criterion);
            }
        }
    }

    criteria
}

/// Parse a single criterion line.
fn parse_criterion_line(line: &str) -> Option<Criterion> {
    // Match - [ ] or - [x] patterns
    if let Some(description) = line.strip_prefix("- [ ] ") {
        Some(Criterion {
            description: description.trim().to_string(),
            completed: false,
        })
    } else if line.starts_with("- [x] ") || line.starts_with("- [X] ") {
        Some(Criterion {
            description: line[6..].trim().to_string(),
            completed: true,
        })
    } else {
        None
    }
}

/// Build an Intent from a parsed spec document.
///
/// # Examples
///
/// ```
/// use rightclick::core::logic::intent::{parse_spec_document, build_intent_from_spec};
/// use std::path::PathBuf;
///
/// let content = r#"---
/// id: intent-123
/// status: in_progress
/// created: 2026-02-14T10:00:00Z
/// updated: 2026-02-14T11:00:00Z
/// ---
///
/// ## Add JWT Authentication
///
/// Implement JWT-based authentication for the API.
///
/// ### Acceptance Criteria
///
/// - [ ] Login endpoint returns JWT
/// - [x] Token validation middleware
/// - [ ] Refresh token endpoint
/// "#;
///
/// let doc = parse_spec_document(content).unwrap();
/// let intent = build_intent_from_spec(doc, PathBuf::from("auth.md"), Some("intent-abc".to_string())).unwrap();
///
/// assert_eq!(intent.title, "Add JWT Authentication");
/// assert_eq!(intent.acceptance_criteria.len(), 3);
/// // Frontmatter id takes precedence over the caller-provided fallback.
/// assert_eq!(intent.id, "intent-123");
/// ```
pub fn build_intent_from_spec(
    doc: SpecDocument,
    spec_path: PathBuf,
    fallback_id: Option<String>,
) -> Result<Intent, IntentParseError> {
    let title = extract_title(&doc.content).unwrap_or_else(|| "Untitled Intent".to_string());

    let description = extract_description(&doc.content).unwrap_or_default();
    let acceptance_criteria = extract_acceptance_criteria(&doc.content);

    let now = doc
        .frontmatter
        .updated
        .clone()
        .or_else(|| doc.frontmatter.created.clone())
        .unwrap_or_else(|| "2026-01-01T00:00:00Z".to_string());

    // The id is resolved in priority order: frontmatter → caller-provided
    // fallback → error. The Core never generates ids itself.
    let id = doc
        .frontmatter
        .id
        .clone()
        .or(fallback_id)
        .ok_or_else(|| IntentParseError::MissingField("id".to_string()))?;

    Ok(Intent {
        id,
        title,
        description,
        status: doc.frontmatter.status.unwrap_or(IntentStatus::Draft),
        spec_path,
        workers: doc
            .frontmatter
            .workers
            .iter()
            .map(|w| w.id.clone())
            .collect(),
        acceptance_criteria,
        metadata: doc
            .frontmatter
            .extra
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
            .collect(),
        created_at: doc.frontmatter.created.unwrap_or_else(|| now.clone()),
        updated_at: now,
    })
}

/// Generate default spec content for a new intent.
///
/// # Examples
///
/// ```
/// use rightclick::core::logic::intent::generate_default_spec;
///
/// let content = generate_default_spec("Add Feature X", "2026-02-14T10:00:00Z", "abc123");
/// assert!(content.contains("Add Feature X"));
/// assert!(content.contains("## Description"));
/// assert!(content.contains("## Acceptance Criteria"));
/// assert!(content.contains("id: intent-abc123"));
/// ```
pub fn generate_default_spec(title: &str, now: &str, id_suffix: &str) -> String {
    format!(
        r#"---
id: intent-{id}
status: draft
created: {now}
updated: {now}
workers: []
---

# {title}

## Description

Describe what needs to be implemented...

## Context

- Current state: ...
- Constraints: ...
- References: ...

## Acceptance Criteria

- [ ] First criterion
- [ ] Second criterion
- [ ] Third criterion

## Notes

Additional notes and considerations...
"#,
        id = id_suffix,
        now = now,
        title = title
    )
}

/// Update spec content with new acceptance criteria completion status.
///
/// Returns the updated content with checkboxes marked appropriately.
///
/// # Examples
///
/// ```
/// use rightclick::core::logic::intent::update_criteria_in_content;
///
/// let content = "- [ ] First\n- [ ] Second";
/// let completed = vec!["First".to_string()];
///
/// let updated = update_criteria_in_content(content, &completed);
/// assert!(updated.contains("- [x] First"));
/// assert!(updated.contains("- [ ] Second"));
/// ```
pub fn update_criteria_in_content(content: &str, completed: &[String]) -> String {
    let mut result = String::new();
    let mut in_criteria_section = false;
    let has_criteria_section = {
        let lower = content.to_lowercase();
        lower.contains("acceptance criteria") || lower.contains("## criteria")
    };

    for line in content.lines() {
        let trimmed = line.trim();

        // Detect criteria section
        if trimmed.to_lowercase().contains("acceptance criteria")
            || trimmed.to_lowercase().contains("## criteria")
        {
            in_criteria_section = true;
            result.push_str(line);
            result.push('\n');
            continue;
        }

        // Exit criteria section
        if in_criteria_section && trimmed.starts_with("## ") {
            in_criteria_section = false;
        }

        // Update criterion lines
        if (in_criteria_section || !has_criteria_section)
            && (trimmed.starts_with("- [ ]") || trimmed.starts_with("- [x]"))
        {
            let criterion_text = trimmed[6..].trim();
            let is_completed = completed.iter().any(|c| criterion_text.contains(c));

            if is_completed {
                result.push_str(&line.replace("- [ ]", "- [x]"));
            } else {
                result.push_str(line);
            }
        } else {
            result.push_str(line);
        }

        result.push('\n');
    }

    result.trim_end().to_string()
}

/// Extract metadata from the "Context" section of a spec.
///
/// Parses key-value pairs in the format "- Key: Value".
pub fn extract_context_metadata(content: &str) -> HashMap<String, String> {
    let mut metadata = HashMap::new();
    let mut in_context_section = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.to_lowercase().starts_with("## context")
            || trimmed.to_lowercase().starts_with("### context")
        {
            in_context_section = true;
            continue;
        }

        if in_context_section && trimmed.starts_with("## ") {
            break;
        }

        if in_context_section && trimmed.starts_with("- ") {
            if let Some((key, value)) = trimmed[2..].split_once(":") {
                metadata.insert(
                    key.trim().to_lowercase().replace(" ", "_"),
                    value.trim().to_string(),
                );
            }
        }
    }

    metadata
}

/// Validate that a spec document is well-formed.
///
/// Returns Ok(()) if valid, or an error describing the problem.
pub fn validate_spec(content: &str) -> Result<(), IntentParseError> {
    // Must have frontmatter
    if !content.starts_with("---") {
        return Err(IntentParseError::InvalidMarkdown(
            "Spec must start with YAML frontmatter".to_string(),
        ));
    }

    let doc = parse_spec_document(content)?;

    // Must have a title
    if extract_title(&doc.content).is_none() {
        return Err(IntentParseError::MissingField(
            "Title (H1 heading) is required".to_string(),
        ));
    }

    // Should have acceptance criteria
    if extract_acceptance_criteria(&doc.content).is_empty() {
        return Err(IntentParseError::InvalidMarkdown(
            "At least one acceptance criterion is recommended".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_spec_document_with_frontmatter() {
        let content = r#"---
id: test-123
status: draft
---

# Test Title

Description here.
"#;

        let doc = parse_spec_document(content).unwrap();
        assert_eq!(doc.frontmatter.id, Some("test-123".to_string()));
        assert_eq!(doc.frontmatter.status, Some(IntentStatus::Draft));
        assert!(doc.content.contains("Test Title"));
    }

    #[test]
    fn test_parse_spec_document_without_frontmatter() {
        let content = "# Just Markdown\n\nNo frontmatter here.";

        let doc = parse_spec_document(content).unwrap();
        assert!(doc.frontmatter.id.is_none());
        assert_eq!(doc.content, content);
    }

    #[test]
    fn test_parse_spec_document_invalid_frontmatter() {
        let content = "---\ninvalid: yaml: :::\n---\n\n# Title";

        let result = parse_spec_document(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_title() {
        assert_eq!(
            extract_title("# My Title\n\nContent"),
            Some("My Title".to_string())
        );
        assert_eq!(
            extract_title("# Title with spaces  \n\nContent"),
            Some("Title with spaces".to_string())
        );
        assert_eq!(extract_title("No heading here"), None);
        assert_eq!(extract_title("## Not H1\n\nContent"), None);
    }

    #[test]
    fn test_extract_description() {
        let content = "# Title\n\nFirst paragraph.\n\nSecond paragraph.";
        assert_eq!(
            extract_description(content),
            Some("First paragraph.".to_string())
        );
    }

    #[test]
    fn test_extract_acceptance_criteria() {
        let content = r#"
# Title

## Acceptance Criteria

- [ ] First thing
- [x] Second thing done
- [ ] Third thing

## Other Section

More content.
"#;

        let criteria = extract_acceptance_criteria(content);
        assert_eq!(criteria.len(), 3);
        assert_eq!(criteria[0].description, "First thing");
        assert!(!criteria[0].completed);
        assert_eq!(criteria[1].description, "Second thing done");
        assert!(criteria[1].completed);
    }

    #[test]
    fn test_generate_default_spec() {
        let spec = generate_default_spec("Test Feature", "2026-02-14T10:00:00Z", "test-suffix");

        assert!(spec.contains("# Test Feature"));
        assert!(spec.contains("## Description"));
        assert!(spec.contains("## Acceptance Criteria"));
        assert!(spec.contains("- [ ] First criterion"));
        assert!(spec.contains("status: draft"));
    }

    #[test]
    fn test_update_criteria_in_content() {
        let content = "- [ ] First\n- [ ] Second\n- [ ] Third";
        let completed = vec!["First".to_string(), "Third".to_string()];

        let updated = update_criteria_in_content(content, &completed);
        assert!(updated.contains("- [x] First"));
        assert!(updated.contains("- [ ] Second"));
        assert!(updated.contains("- [x] Third"));
    }

    #[test]
    fn test_extract_context_metadata() {
        let content = r#"
## Context

- Current State: Using basic auth
- Target: JWT tokens
- Priority: High

## Other

More content.
"#;

        let metadata = extract_context_metadata(content);
        assert_eq!(
            metadata.get("current_state"),
            Some(&"Using basic auth".to_string())
        );
        assert_eq!(metadata.get("target"), Some(&"JWT tokens".to_string()));
        assert_eq!(metadata.get("priority"), Some(&"High".to_string()));
    }

    #[test]
    fn test_validate_spec_success() {
        let content = r#"---
id: test
---

# Valid Title

## Acceptance Criteria

- [ ] One criterion
"#;

        assert!(validate_spec(content).is_ok());
    }

    #[test]
    fn test_validate_spec_no_frontmatter() {
        let content = "# Title\n\nNo frontmatter.";
        assert!(validate_spec(content).is_err());
    }

    #[test]
    fn test_validate_spec_no_title() {
        let content = "---\nid: test\n---\n\nNo H1 heading.";
        assert!(validate_spec(content).is_err());
    }
}
