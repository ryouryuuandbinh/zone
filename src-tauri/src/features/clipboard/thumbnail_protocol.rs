use tauri::{
    http::{header, HeaderValue, Method, Request, Response, StatusCode},
    Manager, UriSchemeContext, UriSchemeResponder, Wry,
};

use super::{valid_id, ClipboardManager};

fn thumbnail_request_id<'a>(
    webview_label: &str,
    request: &'a Request<Vec<u8>>,
) -> Result<&'a str, StatusCode> {
    if webview_label != "island" {
        return Err(StatusCode::FORBIDDEN);
    }
    if request.method() != Method::GET {
        return Err(StatusCode::METHOD_NOT_ALLOWED);
    }
    if request.uri().query().is_some() {
        return Err(StatusCode::NOT_FOUND);
    }
    request
        .uri()
        .path()
        .strip_prefix('/')
        .filter(|id| valid_id(id))
        .ok_or(StatusCode::NOT_FOUND)
}

fn respond(responder: UriSchemeResponder, status: StatusCode, bytes: Vec<u8>) {
    let mut response = Response::new(bytes);
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    if status == StatusCode::OK {
        headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
    } else if status == StatusCode::METHOD_NOT_ALLOWED {
        headers.insert(header::ALLOW, HeaderValue::from_static("GET"));
    }
    responder.respond(response);
}

pub(crate) fn serve_thumbnail(
    context: UriSchemeContext<'_, Wry>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let id = match thumbnail_request_id(context.webview_label(), &request) {
        Ok(id) => id.to_owned(),
        Err(status) => {
            respond(responder, status, Vec::new());
            return;
        }
    };
    let app = context.app_handle().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = app
            .try_state::<ClipboardManager>()
            .and_then(|clipboard| clipboard.thumbnail(&id).ok());
        match bytes {
            Some(bytes) => respond(responder, StatusCode::OK, bytes),
            None => respond(responder, StatusCode::NOT_FOUND, Vec::new()),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thumbnail_route_accepts_only_island_get_for_exact_item_id() {
        let request = |method, uri| {
            Request::builder()
                .method(method)
                .uri(uri)
                .body(Vec::new())
                .unwrap()
        };
        let get = request(Method::GET, "/clip-123-456");
        assert_eq!(thumbnail_request_id("island", &get), Ok("clip-123-456"));
        assert_eq!(
            thumbnail_request_id("settings", &get),
            Err(StatusCode::FORBIDDEN)
        );
        assert_eq!(
            thumbnail_request_id("island", &request(Method::POST, "/clip-123-456")),
            Err(StatusCode::METHOD_NOT_ALLOWED)
        );
        for path in [
            "/clip-123-456.png",
            "/clip-123-456?path=other",
            "/clip-123-456/other",
            "/%2e%2e/clip-123-456",
        ] {
            assert_eq!(
                thumbnail_request_id("island", &request(Method::GET, path)),
                Err(StatusCode::NOT_FOUND)
            );
        }
    }
}
