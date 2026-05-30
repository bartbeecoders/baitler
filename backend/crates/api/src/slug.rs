//! Shared URL-slug generation.
//!
//! A slug is a lowercase, ASCII-alphanumeric, dash-separated rendering of a
//! title, made unique within an owner's namespace for a given table (with a
//! numeric collision suffix). Lifted out of `knowledge/repo.rs` (Phase 11) when
//! pages (Phase 12) needed the same generator over a different table.

use uuid::Uuid;

use crate::db::Db;
use crate::error::AppResult;

/// Lowercase, ASCII-alnum, dash-separated slug. Empty input → `fallback`.
pub fn slugify(name: &str, fallback: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    let slug = out.trim_matches('-').to_string();
    if slug.is_empty() {
        fallback.to_string()
    } else {
        slug
    }
}

/// Pick a slug free within `owner`'s namespace for `table`, appending `-2`,
/// `-3`, … on collision.
///
/// `table` is interpolated into the query, so callers MUST pass a trusted
/// literal table name (`"project"`, `"page"`), never user input.
pub async fn unique_slug(db: &Db, owner: &str, table: &str, base: &str) -> AppResult<String> {
    let sql = format!(
        "SELECT VALUE slug FROM {table} WHERE owner = $owner AND \
         (slug = $base OR string::starts_with(slug, $prefix))"
    );
    let mut res = db
        .query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("base", base.to_string()))
        .bind(("prefix", format!("{base}-")))
        .await?
        .check()?;
    let taken: Vec<String> = res.take(0)?;
    if !taken.contains(&base.to_string()) {
        return Ok(base.to_string());
    }
    for n in 2..10_000 {
        let cand = format!("{base}-{n}");
        if !taken.contains(&cand) {
            return Ok(cand);
        }
    }
    // Pathological fallback: a uuid suffix is guaranteed unique.
    Ok(format!("{base}-{}", Uuid::new_v4()))
}

/// Append short unguessable entropy to a slug — used for `unlisted` pages so the
/// share URL can't be enumerated (the slug itself is the only access control).
pub fn with_entropy(base: &str) -> String {
    // 8 hex chars from a fresh uuid — plenty of unguessability, stays readable.
    let uuid = Uuid::new_v4().simple().to_string();
    format!("{base}-{}", &uuid[..8])
}

#[cfg(test)]
mod tests {
    use super::{slugify, with_entropy};

    #[test]
    fn slugify_basics() {
        assert_eq!(slugify("My Cool Project!", "project"), "my-cool-project");
        assert_eq!(slugify("  spaced  out  ", "project"), "spaced-out");
        assert_eq!(slugify("Rust 2.0 — notes", "page"), "rust-2-0-notes");
        assert_eq!(slugify("***", "project"), "project");
        assert_eq!(slugify("", "page"), "page");
    }

    #[test]
    fn entropy_is_appended_and_varies() {
        let a = with_entropy("hello");
        let b = with_entropy("hello");
        assert!(a.starts_with("hello-"));
        assert_eq!(a.len(), "hello-".len() + 8);
        assert_ne!(a, b, "entropy should differ between calls");
    }
}
