use std::net::SocketAddr;

use axum::{Router, routing::get, response::IntoResponse};
use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer};

async fn health_check() -> impl IntoResponse {
    "OK"
}

#[derive(Debug, thiserror::Error)]
pub enum WebServerError {
    #[error("StdIoError reason = {0}")]
    StdIoError(#[from] std::io::Error)
}

pub struct WebServer {
    task_handle: tokio::task::JoinHandle<Result<(), std::io::Error>>,
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
    pub address: SocketAddr
}

impl WebServer {
    pub async fn run() -> Result<Self, WebServerError> {
        let app = Self::build_router()
            .layer(TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().include_headers(true))
                .on_request(DefaultOnRequest::new().level(tracing::Level::DEBUG))
                .on_response(DefaultOnResponse::new().level(tracing::Level::DEBUG)),
            );

        let listener = tokio::net::TcpListener::bind("0.0.0.0:8080".to_string()).await?;
        let address = listener.local_addr()?;

        tracing::info!("{}({}) listening on {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"), address);

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

        let task_handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    shutdown_rx.await.ok();
                })
                .await
        });

        Ok(Self {
            task_handle,
            shutdown_tx,
            address,
        })
    }

    fn build_router() -> Router {
        Router::new()
            .route("/health", get(health_check))
    }
}