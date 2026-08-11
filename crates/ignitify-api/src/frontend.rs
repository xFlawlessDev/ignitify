use axum::{
    body::Body,
    http::{
        HeaderName, HeaderValue, Method, StatusCode, Uri,
        header::{CACHE_CONTROL, CONTENT_TYPE},
    },
    response::Response,
};

mod assets {
    include!(concat!(env!("OUT_DIR"), "/frontend_assets.rs"));
}

mod build_config {
    include!(concat!(env!("OUT_DIR"), "/frontend_build_config.rs"));
}

const INDEX_DOCUMENT: &str = "index.html";

pub(crate) fn template_catalog_url() -> &'static str {
    build_config::TEMPLATE_CATALOG_URL
}

/// Serves the embedded control-plane SPA after the API routes have been matched.
pub(crate) async fn serve(method: Method, uri: Uri) -> Response {
    if method != Method::GET && method != Method::HEAD {
        return empty_response(StatusCode::METHOD_NOT_ALLOWED);
    }

    let path = uri.path();
    if path == "/api" || path.starts_with("/api/") {
        return empty_response(StatusCode::NOT_FOUND);
    }

    let Some(asset_path) = requested_asset_path(path) else {
        return empty_response(StatusCode::NOT_FOUND);
    };

    if let Some(response) = embedded_file_response(asset_path, &method) {
        return response;
    }
    if is_asset_path(asset_path) {
        return empty_response(StatusCode::NOT_FOUND);
    }

    match embedded_file_response(INDEX_DOCUMENT, &method) {
        Some(response) => response,
        None => empty_response(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

fn requested_asset_path(path: &str) -> Option<&str> {
    let path = path.strip_prefix('/').unwrap_or_default();
    if path.is_empty() {
        return Some(INDEX_DOCUMENT);
    }
    if path
        .split('/')
        .any(|segment| matches!(segment, "." | "..") || segment.contains('\\'))
    {
        return None;
    }
    Some(path)
}

fn embedded_file_response(path: &str, method: &Method) -> Option<Response> {
    let (_, contents) = assets::EMBEDDED_FRONTEND
        .iter()
        .find(|(asset_path, _)| *asset_path == path)?;
    let body = if *method == Method::HEAD {
        Body::empty()
    } else {
        Body::from(*contents)
    };
    let mut response = Response::new(body);
    let headers = response.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(content_type(path)));
    headers.insert(CACHE_CONTROL, HeaderValue::from_static(cache_control(path)));
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    Some(response)
}

fn empty_response(status: StatusCode) -> Response {
    let mut response = Response::new(Body::empty());
    *response.status_mut() = status;
    response
}

fn is_asset_path(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|segment| segment.contains('.'))
}

fn cache_control(path: &str) -> &'static str {
    if path == INDEX_DOCUMENT {
        "no-cache"
    } else {
        "public, max-age=31536000, immutable"
    }
}

fn content_type(path: &str) -> &'static str {
    if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".js") {
        "text/javascript; charset=utf-8"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".webp") {
        "image/webp"
    } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        "image/jpeg"
    } else if path.ends_with(".ico") {
        "image/x-icon"
    } else if path.ends_with(".woff2") {
        "font/woff2"
    } else if path.ends_with(".woff") {
        "font/woff"
    } else if path.ends_with(".json") || path.ends_with(".webmanifest") {
        "application/json"
    } else {
        "text/html; charset=utf-8"
    }
}

#[cfg(test)]
mod tests {
    use axum::http::{Method, StatusCode, Uri, header};

    use super::serve;

    #[tokio::test]
    async fn serves_the_embedded_spa_for_root_and_deep_links() {
        for path in ["/", "/projects/project-1"] {
            let response = serve(Method::GET, Uri::from_static(path)).await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.headers().get(header::CONTENT_TYPE),
                Some(&header::HeaderValue::from_static(
                    "text/html; charset=utf-8"
                ))
            );
            assert_eq!(
                response.headers().get(header::CACHE_CONTROL),
                Some(&header::HeaderValue::from_static("no-cache"))
            );
        }
    }

    #[tokio::test]
    async fn does_not_fall_back_to_the_spa_for_api_or_missing_assets() {
        for path in ["/api/v1/unknown", "/assets/missing.js", "/../index.html"] {
            let response = serve(Method::GET, Uri::from_static(path)).await;
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }
    }

    #[tokio::test]
    async fn only_allows_safe_static_asset_methods() {
        let response = serve(Method::POST, Uri::from_static("/")).await;
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }
}
