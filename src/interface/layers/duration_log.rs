use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use tokio::time::Instant;
use tracing::Instrument;

pub async fn handle(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let span = tracing::info_span!("http_request", %method, %uri);

    async move {
        let start = Instant::now();
        let response = next.run(req).await;
        let status = response.status().as_u16();
        let elapsed = start.elapsed();

        if status >= 500 {
            tracing::error!(status, ?elapsed, "request failed with server error");
        } else if status >= 400 {
            tracing::warn!(status, ?elapsed, "request rejected with client error");
        } else {
            tracing::info!(status, ?elapsed, "request completed");
        }

        response
    }
    .instrument(span)
    .await
}
