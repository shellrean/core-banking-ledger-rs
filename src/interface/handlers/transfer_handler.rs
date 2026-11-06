use axum::extract::State;
use axum::Json;
use crate::application::dto::transfer_dto::{
    DepositRequest, SettlementResponse, TransferRequest, TransferResponse,
};
use crate::error::AppError;
use crate::interface::state::AppState;

pub async fn deposit(
    State(state): State<AppState>,
    Json(payload): Json<DepositRequest>,
) -> Result<Json<TransferResponse>, AppError> {
    let receipt = state.ledger_service.deposit(payload).await?;
    Ok(Json(receipt))
}

pub async fn execute(
    State(state): State<AppState>,
    Json(payload): Json<TransferRequest>,
) -> Result<Json<TransferResponse>, AppError> {
    let receipt = state.ledger_service.transfer(payload).await?;
    Ok(Json(receipt))
}

pub async fn settle_eod(
    State(state): State<AppState>,
) -> Result<Json<SettlementResponse>, AppError> {
    let report = state.ledger_service.settle_eod().await?;
    Ok(Json(report))
}
