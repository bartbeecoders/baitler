//! Schema migration runner.
//!
//! Migration files live in `migrations/*.surql` (relative to this crate) and are
//! embedded into the binary at compile time, so they apply identically whether
//! the server runs from source, a release binary, or a test harness — no
//! dependence on the current working directory.
//!
//! Files are applied in lexicographic filename order. Each file's statements and
//! its bookkeeping row are committed in a single `BEGIN/COMMIT` transaction, so
//! "apply" and "record as applied" succeed or fail together — a failed migration
//! never leaves a half-applied file that the next boot would re-run. The
//! `_migration` table records which files have run.

use include_dir::{include_dir, Dir};
use thiserror::Error;

use crate::db::Db;

/// All `.surql` files under `crates/api/migrations`, embedded at compile time.
static MIGRATIONS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/migrations");

/// Errors that can occur while applying migrations.
#[derive(Debug, Error)]
pub enum MigrationError {
    #[error("migration file `{file}` is not valid UTF-8")]
    NotUtf8 { file: String },

    #[error(transparent)]
    Db(#[from] surrealdb::Error),
}

/// Apply every embedded migration in filename order.
pub async fn run(db: &Db) -> Result<(), MigrationError> {
    // Ensure the bookkeeping table exists before we consult it.
    db.query("DEFINE TABLE IF NOT EXISTS _migration SCHEMALESS")
        .await?
        .check()?;

    let mut files: Vec<_> = MIGRATIONS
        .files()
        .filter(|f| f.path().extension().is_some_and(|ext| ext == "surql"))
        .collect();
    files.sort_by_key(|f| f.path().to_path_buf());

    for file in files {
        let name = file.path().to_string_lossy().to_string();

        // Skip migrations already recorded as applied.
        let mut already = db
            .query("SELECT VALUE id FROM _migration WHERE name = $name")
            .bind(("name", name.clone()))
            .await?;
        let applied: Vec<surrealdb::RecordId> = already.take(0)?;
        if !applied.is_empty() {
            tracing::debug!(migration = %name, "already applied, skipping");
            continue;
        }

        let body = file
            .contents_utf8()
            .ok_or_else(|| MigrationError::NotUtf8 { file: name.clone() })?
            .trim();

        // Commit the migration body and its bookkeeping row atomically. Ensuring
        // the body ends with `;` avoids gluing it to the CREATE statement.
        let terminated_body = if body.ends_with(';') {
            body.to_string()
        } else {
            format!("{body};")
        };
        let transaction = format!(
            "BEGIN TRANSACTION;\n\
             {terminated_body}\n\
             CREATE _migration SET name = $name, applied_at = time::now();\n\
             COMMIT TRANSACTION;"
        );

        tracing::info!(migration = %name, "applying migration");
        db.query(transaction).bind(("name", name)).await?.check()?;
    }

    Ok(())
}
