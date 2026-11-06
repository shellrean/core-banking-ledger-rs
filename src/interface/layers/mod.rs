use axum::{middleware, Router};
use crate::interface::state::AppState;

pub mod duration_log;

pub fn apply(state: AppState, router: Router<AppState>) -> Router {
    router
        .layer(middleware::from_fn(duration_log::handle))
        .with_state(state)
}
