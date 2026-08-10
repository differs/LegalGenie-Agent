//! Idempotency-Key support for write endpoints (P0).
//!
//! Clients may send an `Idempotency-Key` header on POST/PUT/PATCH/DELETE
//! requests. The middleware records the response of the first successful
//! attempt keyed by `sha256(method|path|key|authorization)` and replays it
//! verbatim on retries, so network-level retries cannot create duplicates.
//!
//! Only 2xx responses are cached; failures are never replayed so clients can
//! retry and fix the underlying problem.
use crate::state::AppState;
use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, HeaderValue, Method},
    middleware::Next,
    response::Response,
};
use http_body_util::BodyExt;
use sha2::{Digest, Sha256};

const IDEMPOTENCY_KEY_HEADER: &str = "idempotency-key";

pub async fn idempotency_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let method = req.method().clone();
    if !matches!(
        method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    ) {
        return next.run(req).await;
    }

    let Some(key_header) = req.headers().get(IDEMPOTENCY_KEY_HEADER) else {
        return next.run(req).await;
    };
    let key_header = key_header.to_str().unwrap_or("").to_string();
    if key_header.trim().is_empty() || key_header.len() > 128 {
        return next.run(req).await;
    }

    let path = req.uri().path().to_string();
    let auth = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let mut hasher = Sha256::new();
    hasher.update(method.as_str());
    hasher.update(b"|");
    hasher.update(path.as_bytes());
    hasher.update(b"|");
    hasher.update(key_header.as_bytes());
    hasher.update(b"|");
    hasher.update(auth.as_bytes());
    let request_key = hex_encode(&hasher.finalize());

    // Replay a previously cached successful response if present.
    let replay = sqlx::query_as::<_, (i64, String)>(
        "SELECT status_code, response_body FROM idempotency_records WHERE request_key = ?1",
    )
    .bind(&request_key)
    .fetch_optional(&state.pool)
    .await;

    match replay {
        Ok(Some((status_code, body))) => {
            let status = axum::http::StatusCode::from_u16(status_code as u16)
                .unwrap_or(axum::http::StatusCode::OK);
            return Response::builder()
                .status(status)
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-idempotency-replayed", "true")
                .body(Body::from(body))
                .expect("build replayed response");
        }
        Ok(None) => {}
        Err(err) => {
            // A DB failure must not break the request path; proceed without
            // idempotency protection rather than failing the write.
            tracing::warn!(error = %err, "idempotency lookup failed; proceeding unprotected");
        }
    }

    let response = next.run(req).await;

    if !response.status().is_success() {
        return response;
    }

    // Cache the successful response body for replays.
    let (parts, body) = response.into_parts();
    match body.collect().await {
        Ok(collected) => {
            let bytes = collected.to_bytes();
            let body_str = String::from_utf8_lossy(&bytes).into_owned();
            let status_code = parts.status.as_u16() as i64;

            let result = sqlx::query(
                "INSERT INTO idempotency_records (request_key, status_code, response_body) VALUES (?1, ?2, ?3)",
            )
            .bind(&request_key)
            .bind(status_code)
            .bind(&body_str)
            .execute(&state.pool)
            .await;

            if let Err(err) = result {
                tracing::warn!(error = %err, "idempotency record insert failed");
            }

            let mut response = Response::from_parts(parts, Body::from(bytes));
            response.headers_mut().insert(
                "x-idempotency-key",
                HeaderValue::from_str(&key_header).unwrap_or_else(|_| HeaderValue::from_static("")),
            );
            response
        }
        Err(err) => {
            tracing::warn!(error = %err, "idempotency body collection failed");
            Response::from_parts(parts, Body::from(axum::body::Bytes::new()))
        }
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::hex_encode;

    #[test]
    fn hex_encoding_is_lowercase() {
        assert_eq!(hex_encode(&[0xAB, 0xCD]), "abcd");
        assert_eq!(hex_encode(&[0x00, 0xFF]), "00ff");
    }
}
