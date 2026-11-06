pub mod application;
pub mod domain;
pub mod error;
pub mod infrastructure;
pub mod interface;

use std::error::Error;
use std::sync::Arc;
use crate::application::services::{account_service, ledger_service};
use crate::infrastructure::database::{create_pool, run_migrations};
use crate::infrastructure::repositories::{account_repo, ledger_repo};
use crate::interface::routes::create_router;
use crate::interface::state::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let _ = dotenv::dotenv();

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(true)
        .init();

    let pool = match create_pool().await {
        Ok(p) => {
            if let Err(e) = run_migrations(&p).await {
                tracing::warn!(error = %e, "failed to run migrations");
            }
            p
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to create database pool");
            return Err(e.into());
        }
    };

    let account_repository = account_repo::AccountPostgresRepository::new(pool.clone());
    let account_service = account_service::AccountService::new(Arc::new(account_repository));

    let ledger_repository = ledger_repo::LedgerPostgresRepository::new(pool.clone());
    let ledger_service = ledger_service::LedgerService::new(Arc::new(ledger_repository));

    // Spawn background End-of-Day (EOD) Settlement Worker for hot account (SYSTEM_MINT).
    // Optionally override with `EOD_SETTLEMENT_INTERVAL_SECS` for frequent local testing;
    // defaults to running at 00:00:00 UTC daily.
    let eod_interval = std::env::var("EOD_SETTLEMENT_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(std::time::Duration::from_secs);
    ledger_service.spawn_eod_settlement_worker(eod_interval);

    let state = AppState {
        account_service,
        ledger_service,
    };

    let router = create_router(state);

    let addr = std::env::var("SERVER_ADDR").unwrap_or_else(|_| String::from("localhost:9090"));
    tracing::info!(address = %format!("http://{}", addr), "Core Banking Ledger API listening");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
