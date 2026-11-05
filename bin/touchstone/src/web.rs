//! Static web UI: embedded (feature `embed-web`) or served from `web/build` on disk.

use axum::Router;

#[cfg(feature = "embed-web")]
pub fn router() -> Router {
    use axum::http::{StatusCode, Uri, header};
    use axum::response::{IntoResponse, Response};

    #[derive(rust_embed::RustEmbed)]
    #[folder = "../../web/build"]
    struct Assets;

    async fn serve(uri: Uri) -> Response {
        let path = uri.path().trim_start_matches('/');
        // Unknown non-API paths fall back to the SPA shell.
        let (name, file) = match Assets::get(path) {
            Some(f) if !path.is_empty() => (path, f),
            _ if path.starts_with("api/") => return StatusCode::NOT_FOUND.into_response(),
            _ => match Assets::get("index.html") {
                Some(f) => ("index.html", f),
                None => return StatusCode::NOT_FOUND.into_response(),
            },
        };
        let mime = mime_guess::from_path(name).first_or_octet_stream();
        ([(header::CONTENT_TYPE, mime.to_string())], file.data).into_response()
    }

    Router::new().fallback(serve)
}

#[cfg(not(feature = "embed-web"))]
pub fn router() -> Router {
    use disk::serve_dir;
    use std::path::PathBuf;

    // Resolved at run time so debug builds and tests work without a web build.
    let dir = std::env::var_os("TOUCHSTONE_WEB_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("web/build"));
    serve_dir(dir)
}

#[cfg(not(feature = "embed-web"))]
mod disk {
    use std::path::{Component, Path, PathBuf};

    use axum::Router;
    use axum::body::Body;
    use axum::http::{StatusCode, Uri, header};
    use axum::response::{IntoResponse, Response};

    pub fn serve_dir(dir: PathBuf) -> Router {
        Router::new().fallback(move |uri: Uri| {
            let dir = dir.clone();
            async move { respond(&dir, uri.path()).await }
        })
    }

    async fn respond(dir: &Path, req_path: &str) -> Response {
        let rel = Path::new(req_path.trim_start_matches('/'));
        // Reject traversal; only plain components are allowed.
        let safe = rel.components().all(|c| matches!(c, Component::Normal(_)));
        let mut file = dir.join(rel);
        let is_file = safe && !rel.as_os_str().is_empty() && file.is_file();
        if !is_file {
            if req_path.starts_with("/api/") || req_path == "/api" || !safe {
                return StatusCode::NOT_FOUND.into_response();
            }
            file = dir.join("index.html");
        }
        match tokio::fs::read(&file).await {
            Ok(bytes) => {
                let mime = mime_for(&file);
                ([(header::CONTENT_TYPE, mime)], Body::from(bytes)).into_response()
            }
            Err(_) => StatusCode::NOT_FOUND.into_response(),
        }
    }

    fn mime_for(p: &Path) -> &'static str {
        match p.extension().and_then(|e| e.to_str()) {
            Some("html") => "text/html; charset=utf-8",
            Some("js" | "mjs") => "text/javascript",
            Some("css") => "text/css",
            Some("json") => "application/json",
            Some("svg") => "image/svg+xml",
            Some("png") => "image/png",
            Some("ico") => "image/x-icon",
            Some("woff2") => "font/woff2",
            Some("txt") => "text/plain; charset=utf-8",
            _ => "application/octet-stream",
        }
    }
}
