//! Baitler API binary entrypoint.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Load .env if present; real environment variables take precedence.
    let _ = dotenvy::dotenv();

    baitler_api::telemetry::init();

    if let Err(err) = baitler_api::run().await {
        tracing::error!(error = %err, "fatal: server exited with error");
        return Err(err);
    }

    Ok(())
}
