//! Root-jailed access to the local filesystem (`WORKSPACE_ROOTS`).
//!
//! Everything Baitler does on the host disk goes through this module's path
//! resolution: the per-run read-only agent grant, the folder-attach browse
//! picker, and the MCP `workspace_*` file-management tools. The invariant is a
//! single one: **a path is usable only if it canonicalizes to within one of the
//! configured roots** — `..`, symlinks, and look-alike siblings
//! (`/home/bart-evil`) cannot escape, because canonicalization resolves them
//! and [`std::path::Path::starts_with`] compares whole components.
//!
//! Two resolution modes:
//! - [`resolve_under_roots`] — for paths that must already exist (read/list/
//!   delete/move-source).
//! - [`resolve_for_create`] — for paths that may not exist yet (write/mkdir/
//!   move-destination/copy-destination): the **parent** must exist and resolve
//!   under a root, and the final component is appended to the canonical parent.
//!
//! Destructive operations must additionally refuse to touch a root itself
//! ([`is_root`]) so a tool call can't delete or rename a whole allow-listed
//! tree by accident.

use std::path::{Component, Path, PathBuf};

/// Resolve a user-supplied local path against the server's allow-list and return
/// its canonical form. The path must exist. Accepts files or dirs.
pub fn resolve_under_roots(roots: &[String], path: &str) -> Result<PathBuf, String> {
    if roots.is_empty() {
        return Err(
            "local file access is not enabled on this server (set WORKSPACE_ROOTS)".to_string(),
        );
    }
    let target = std::fs::canonicalize(path)
        .map_err(|_| format!("path not found or not accessible: {path}"))?;
    for root in roots {
        if let Ok(croot) = std::fs::canonicalize(root) {
            if target.starts_with(&croot) {
                return Ok(target);
            }
        }
    }
    Err(format!(
        "path is not within an allowed root ({})",
        roots.join(", ")
    ))
}

/// Resolve a path that may not exist yet (a write/mkdir/move/copy destination):
/// the path must be absolute with no `.`/`..` components, its **parent** must
/// exist and resolve within an allowed root, and the final component is appended
/// to the canonical parent (so the result is canonical-parent + name).
pub fn resolve_for_create(roots: &[String], path: &str) -> Result<PathBuf, String> {
    let p = Path::new(path);
    if !p.is_absolute() {
        return Err(format!("path must be absolute: {path}"));
    }
    if p.components()
        .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(format!("path must not contain `.`/`..` segments: {path}"));
    }
    let name = p
        .file_name()
        .ok_or_else(|| format!("path has no file name: {path}"))?;
    let parent = p
        .parent()
        .ok_or_else(|| format!("path has no parent directory: {path}"))?;
    let canon_parent = resolve_under_roots(roots, &parent.to_string_lossy())?;
    if !canon_parent.is_dir() {
        return Err(format!("parent is not a directory: {}", parent.display()));
    }
    Ok(canon_parent.join(name))
}

/// Whether `target` (already canonical) IS one of the configured roots.
/// Destructive operations (delete/rmdir/move-source) must refuse roots.
pub fn is_root(roots: &[String], target: &Path) -> bool {
    roots
        .iter()
        .filter_map(|r| std::fs::canonicalize(r).ok())
        .any(|croot| croot == target)
}

/// Validate a user-requested local **folder** grant (must resolve, within a root,
/// and be a directory).
pub fn validate_workspace(roots: &[String], path: &str) -> Result<PathBuf, String> {
    let target = resolve_under_roots(roots, path)?;
    if !target.is_dir() {
        return Err(format!("not a directory: {path}"));
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh temp root (unique per test) + its config representation.
    fn temp_root(tag: &str) -> (PathBuf, Vec<String>) {
        let root = std::env::temp_dir().join(format!("baitler-ws-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let roots = vec![root.to_string_lossy().into_owned()];
        (root, roots)
    }

    #[test]
    fn existing_paths_resolve_only_under_roots() {
        let (root, roots) = temp_root("exist");
        let sub = root.join("sub");
        std::fs::create_dir_all(&sub).unwrap();

        assert!(resolve_under_roots(&roots, &sub.to_string_lossy()).is_ok());
        assert!(resolve_under_roots(&roots, &root.to_string_lossy()).is_ok());
        // Outside the root, traversal, non-existent, and no-roots all fail.
        assert!(resolve_under_roots(&roots, &std::env::temp_dir().to_string_lossy()).is_err());
        assert!(
            resolve_under_roots(&roots, &format!("{}/sub/../..", root.to_string_lossy())).is_err()
        );
        assert!(resolve_under_roots(&roots, &root.join("nope").to_string_lossy()).is_err());
        assert!(resolve_under_roots(&[], &sub.to_string_lossy()).is_err());
    }

    #[test]
    fn create_resolution_requires_clean_absolute_paths_with_existing_parent() {
        let (root, roots) = temp_root("create");

        // New name under the root: OK, returns canonical-parent + name.
        let ok = resolve_for_create(&roots, &root.join("new.txt").to_string_lossy()).unwrap();
        assert!(ok.starts_with(std::fs::canonicalize(&root).unwrap()));

        // `..` segments are rejected outright (even ones that would stay inside).
        assert!(resolve_for_create(
            &roots,
            &format!("{}/sub/../new.txt", root.to_string_lossy())
        )
        .is_err());
        // Relative paths are rejected.
        assert!(resolve_for_create(&roots, "relative.txt").is_err());
        // Parent must exist (no implicit deep-create).
        assert!(
            resolve_for_create(&roots, &root.join("missing/new.txt").to_string_lossy()).is_err()
        );
        // Parent outside the roots is rejected.
        assert!(resolve_for_create(
            &roots,
            &std::env::temp_dir().join("evil.txt").to_string_lossy()
        )
        .is_err());
    }

    #[test]
    fn symlink_escape_is_caught() {
        let (root, roots) = temp_root("link");
        let outside =
            std::env::temp_dir().join(format!("baitler-ws-outside-{}", std::process::id()));
        std::fs::create_dir_all(&outside).unwrap();
        let link = root.join("escape");
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(&outside, &link).unwrap();

        // The symlink target resolves outside the root → rejected, both for
        // existing-path resolution and as a create parent.
        assert!(resolve_under_roots(&roots, &link.to_string_lossy()).is_err());
        assert!(resolve_for_create(&roots, &link.join("new.txt").to_string_lossy()).is_err());
    }

    #[test]
    fn roots_are_recognized_for_protection() {
        let (root, roots) = temp_root("rootprot");
        let sub = root.join("sub");
        std::fs::create_dir_all(&sub).unwrap();

        let canon_root = std::fs::canonicalize(&root).unwrap();
        let canon_sub = std::fs::canonicalize(&sub).unwrap();
        assert!(is_root(&roots, &canon_root));
        assert!(!is_root(&roots, &canon_sub));
    }

    #[test]
    fn workspace_grant_requires_directory() {
        let (root, roots) = temp_root("grant");
        let file = root.join("f.txt");
        std::fs::write(&file, b"x").unwrap();

        assert!(validate_workspace(&roots, &root.to_string_lossy()).is_ok());
        assert!(validate_workspace(&roots, &file.to_string_lossy()).is_err());
    }
}
