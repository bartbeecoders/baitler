//! Shared tag normalization (Phase 14).
//!
//! Lifted out of `ideas` so ideas, documents, and pages share one tag rule. Tags
//! are trimmed, empties dropped, deduped (author casing + order preserved), and
//! capped by per-tag length and total count. Kept faithful to the original
//! `ideas` behaviour so the lift is a pure refactor.

use crate::error::{AppError, AppResult};

/// Maximum number of tags on one item.
pub const MAX_TAGS: usize = 50;
/// Maximum characters in a single tag.
pub const MAX_TAG_LEN: usize = 50;

/// Normalize a tag list, rejecting an over-long tag or too many tags with a 400.
pub fn normalize_tags(tags: Vec<String>) -> AppResult<Vec<String>> {
    let mut cleaned: Vec<String> = Vec::new();
    for tag in tags {
        let trimmed = tag.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.chars().count() > MAX_TAG_LEN {
            return Err(AppError::BadRequest("a tag is too long".into()));
        }
        let tag = trimmed.to_string();
        if !cleaned.contains(&tag) {
            cleaned.push(tag);
        }
    }
    if cleaned.len() > MAX_TAGS {
        return Err(AppError::BadRequest("too many tags".into()));
    }
    Ok(cleaned)
}

#[cfg(test)]
mod tests {
    use super::normalize_tags;

    #[test]
    fn trims_drops_empty_and_dedupes() {
        let out = normalize_tags(vec![
            "  work ".into(),
            "work".into(),
            "".into(),
            "  ".into(),
            "q3".into(),
        ])
        .unwrap();
        assert_eq!(out, vec!["work".to_string(), "q3".to_string()]);
    }

    #[test]
    fn rejects_too_long_and_too_many() {
        assert!(normalize_tags(vec!["x".repeat(51)]).is_err());
        let many: Vec<String> = (0..51).map(|i| format!("t{i}")).collect();
        assert!(normalize_tags(many).is_err());
    }
}
