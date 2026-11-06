use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use uuid::Uuid;
use crate::application::dto::account_dto::{AccountResponse, CreateAccountRequest};
use crate::error::AppError;
use crate::interface::state::AppState;

pub async fn list(
    State(state): State<AppState>,
) -> Result<Json<Vec<AccountResponse>>, AppError> {
    let accounts = state.account_service.list_accounts().await?;
    Ok(Json(accounts))
}

pub async fn get_by_id(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<AccountResponse>, AppError> {
    let account = state.account_service.get_account(id).await?;
    Ok(Json(account))
}

pub async fn create(
    State(state): State<AppState>,
    Json(payload): Json<CreateAccountRequest>,
) -> Result<(StatusCode, Json<AccountResponse>), AppError> {
    let account = state.account_service.create_account(payload).await?;
    Ok((StatusCode::CREATED, Json(account)))
}
