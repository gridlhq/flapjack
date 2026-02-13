use axum::{extract::Request, http::header::CONTENT_TYPE, middleware::Next, response::Response};

pub async fn normalize_content_type(mut request: Request, next: Next) -> Response {
    if request.method() == axum::http::Method::POST || request.method() == axum::http::Method::PUT {
        request
            .headers_mut()
            .insert(CONTENT_TYPE, "application/json".parse().unwrap());
    }
    next.run(request).await
}

/// Chrome 142+ Private Network Access: when a public HTTPS site fetches localhost,
/// the preflight includes `Access-Control-Request-Private-Network: true`.
/// The server must respond with `Access-Control-Allow-Private-Network: true`.
pub async fn allow_private_network(request: Request, next: Next) -> Response {
    let needs_pna = request
        .headers()
        .get("access-control-request-private-network")
        .is_some();
    let mut response = next.run(request).await;
    if needs_pna {
        response.headers_mut().insert(
            "access-control-allow-private-network",
            "true".parse().unwrap(),
        );
    }
    response
}
