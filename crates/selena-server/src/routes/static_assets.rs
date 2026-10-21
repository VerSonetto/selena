use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../web/dist"]
struct Assets;

pub async fn spa_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path.starts_with("api/") || path.starts_with("v1/") {
        return StatusCode::NOT_FOUND.into_response();
    }

    if let Some(asset) = Assets::get(path) {
        return asset_response(path, asset);
    }
    if let Some(asset) = Assets::get("index.html") {
        return asset_response("index.html", asset);
    }
    (
        StatusCode::SERVICE_UNAVAILABLE,
        "Selena web assets are not built. Run `pnpm build` in web/.",
    )
        .into_response()
}

fn asset_response(path: &str, asset: rust_embed::EmbeddedFile) -> Response {
    let mime = match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("woff2") => "font/woff2",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    };
    let cache = if path == "index.html" {
        "no-cache"
    } else {
        "public, max-age=31536000, immutable"
    };
    (
        [(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, cache)],
        asset.data.into_owned(),
    )
        .into_response()
}
