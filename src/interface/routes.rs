use axum::Router;
use axum::routing::{get, post};
use crate::interface::handlers::{account_handler, transfer_handler};
use crate::interface::layers;
use crate::interface::state::AppState;

pub fn create_router(state: AppState) -> Router {
    let router = Router::new()
        .route("/accounts", get(account_handler::list).post(account_handler::create))
        .route("/accounts/{id}", get(account_handler::get_by_id))
        .route("/deposits", post(transfer_handler::deposit))
        .route("/transfers", post(transfer_handler::execute))
        .route("/settlements/eod", post(transfer_handler::settle_eod));

    layers::apply(state, router)
}
