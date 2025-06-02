use std::{net::SocketAddr, sync::Arc};

use axum::{extract::{self, DefaultBodyLimit}, http::StatusCode, response::{Html, IntoResponse}, routing::{get, post}, Router};
use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer};

use crate::{FilesManagerSink, FilesSource, FilesSourceHandler, FilesSourceType};

pub struct WebServerAppData {
    file_sender: tokio::sync::mpsc::Sender<FilesSourceType>,
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
            file_sender: sink.get_tx().clone()
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
    const MAX_VIDEO_FILESIZE_BYTES: usize = 1024 * 1024 * 100;
    async fn health_check() -> impl IntoResponse {
        "OK"
    }

    async fn upload_form() -> Html<&'static str> {
        Html(r#"
            <!DOCTYPE html>
            <html>
            <head><title>Upload Video</title></head>
            <body>
                <h1>Upload Video File (max 100MB)</h1>
                <form action="/upload" method="post" enctype="multipart/form-data">
                    <input type="file" name="file" accept="video/*" required />
                    <button type="submit">Upload</button>
                </form>
            </body>
            </html>
        "#)
    }

    async fn upload_video(extract::State(
        app_data): extract::State<Arc<WebServerAppData>>,
        mut multipart: extract::Multipart
    ) -> impl IntoResponse {
        while let Some(field) = multipart.next_field().await.unwrap_or(None) {
            if let Some(name) = field.name() {
                if name != "file" {
                    println!("Bad field name '{name}'.");
                    continue;
                }
                
                let filename = if let Some(filename) = field.file_name() {
                    filename.to_string()
                } else {
                    tracing::warn!("Missing filename in field {field:?}.");
                    continue;
                };

                let data = match field.bytes().await {
                    Ok(data) => data,
                    Err(_) => return (StatusCode::BAD_REQUEST, "Failed to read file").into_response(),
                };

                // Send to file manager
                if app_data.file_sender.send(FilesSourceType::UploadedVideo { filename, data }).await.is_err() {
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to forward file").into_response();
                }

                return (StatusCode::OK, "Uploaded").into_response();
            } else {
                println!("Field missing name!")
            }
        }

        (StatusCode::BAD_REQUEST, "No file field").into_response()
    }

    fn build_router(app_data: Arc<WebServerAppData>) -> Router {
        Router::new()
            .route("/health", get(Self::health_check))
            .route("/upload", post(Self::upload_video))
                .layer(DefaultBodyLimit::max(Self::MAX_VIDEO_FILESIZE_BYTES))
            .route("/upload", get(Self::upload_form))
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
