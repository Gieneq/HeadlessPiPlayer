use std::{net::SocketAddr, sync::Arc};

use axum::{Router, routing::get, response::IntoResponse};
use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer};

use crate::{FilesManagerSink, FilesSource, FilesSourceHandler};

pub struct WebServerAppData {
    sink: Arc<dyn FilesManagerSink>,
}

#[derive(Debug, thiserror::Error)]
pub enum WebServerError {
    #[error("StdIoError reason = {0}")]
    StdIoError(#[from] std::io::Error),

    #[error("TokioJoinError")]
    TokioJoinError(#[from] tokio::task::JoinError),
}

pub struct WebServer;

impl FilesSource for WebServer {
    type Handler = WebServerHandler;
    type Error = WebServerError;

    async fn start(self, sink: Arc<dyn FilesManagerSink>) -> Result<Self::Handler, Self::Error> {
        let app_data = Arc::new(WebServerAppData {
            sink
        });

        let app = Self::build_router(app_data)
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
            let result = axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    shutdown_rx.await.ok();
                })
                .await;
            if let Err(e) = result {
                tracing::error!("Web server error: {e}");
            }
        });

        Ok(Self::Handler {
            task_handle,
            shutdown_tx,
            address,
        })
    }
}

impl WebServer {
    async fn health_check() -> impl IntoResponse {
        "OK"
    }

    fn build_router(app_data: Arc<WebServerAppData>) -> Router {
        Router::new()
            .route("/health", get(Self::health_check))
            .with_state(app_data)
    }
}

pub struct WebServerHandler {
    task_handle: tokio::task::JoinHandle<()>,
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
    pub address: SocketAddr
}

impl FilesSourceHandler for WebServerHandler {
    type Error = WebServerError;
    
    async fn shutdown(self) -> Result<(), Self::Error> {
        if self.shutdown_tx.send(()).is_err() {
            tracing::error!("Could not send shutdown web server signal");
        }
        self.task_handle.await
            .map_err(Self::Error::from)
    }
    
    async fn await_finish(self) -> Result<(), Self::Error> {
        self.task_handle.await
            .map_err(Self::Error::from)
    }
}
