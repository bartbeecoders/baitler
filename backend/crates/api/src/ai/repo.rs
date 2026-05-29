//! Persistence for encrypted provider API keys. Owner-scoped; plaintext keys
//! never touch this layer (callers pass/receive ciphertext).

use crate::db::Db;
use crate::error::AppResult;

/// Store (or replace) the ciphertext for an (owner, provider) key.
pub async fn set_key(db: &Db, owner: &str, provider: &str, ciphertext: &str) -> AppResult<()> {
    // The unique (owner, provider) index forbids duplicates, so replace.
    let sql = "DELETE provider_key WHERE owner = $owner AND provider = $provider; \
               CREATE provider_key CONTENT { owner: $owner, provider: $provider, ciphertext: $ciphertext }";
    db.query(sql)
        .bind(("owner", owner.to_string()))
        .bind(("provider", provider.to_string()))
        .bind(("ciphertext", ciphertext.to_string()))
        .await?
        .check()?;
    Ok(())
}

/// Fetch the stored ciphertext for an (owner, provider) key, if any.
pub async fn get_ciphertext(db: &Db, owner: &str, provider: &str) -> AppResult<Option<String>> {
    let mut res = db
        .query("SELECT VALUE ciphertext FROM provider_key WHERE owner = $owner AND provider = $provider")
        .bind(("owner", owner.to_string()))
        .bind(("provider", provider.to_string()))
        .await?
        .check()?;
    let rows: Vec<String> = res.take(0)?;
    Ok(rows.into_iter().next())
}

/// Delete a stored key. No-op if absent.
pub async fn delete_key(db: &Db, owner: &str, provider: &str) -> AppResult<()> {
    db.query("DELETE provider_key WHERE owner = $owner AND provider = $provider")
        .bind(("owner", owner.to_string()))
        .bind(("provider", provider.to_string()))
        .await?
        .check()?;
    Ok(())
}

/// Provider ids that have a stored key for this owner.
pub async fn list_configured(db: &Db, owner: &str) -> AppResult<Vec<String>> {
    let mut res = db
        .query("SELECT VALUE provider FROM provider_key WHERE owner = $owner")
        .bind(("owner", owner.to_string()))
        .await?
        .check()?;
    Ok(res.take(0)?)
}
