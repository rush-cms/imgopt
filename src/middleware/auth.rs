use axum::{
    body::Body,
    http::{Request, Response, StatusCode},
    response::IntoResponse,
};
use std::env;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use subtle::ConstantTimeEq;
use tower::{Layer, Service};

#[derive(Clone)]
pub struct AuthLayer;

impl<S> Layer<S> for AuthLayer {
    type Service = AuthService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthService { inner }
    }
}

#[derive(Clone)]
pub struct AuthService<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for AuthService<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        // Skip auth for health/ready probes
        let path = req.uri().path();
        if path == "/health" || path == "/ready" {
            let fut = self.inner.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res)
            });
        }

        // SEC-001: reject if API_TOKEN is not configured or is empty
        let token = match env::var("API_TOKEN") {
            Ok(t) if !t.is_empty() => t,
            _ => {
                tracing::error!("API_TOKEN is not configured or is empty");
                return Box::pin(async move {
                    Ok((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Server configuration error",
                    )
                        .into_response())
                });
            }
        };

        let auth_header = req.headers().get("Authorization");

        let authorized = match auth_header {
            Some(header) => {
                let header_str = header.to_str().unwrap_or("");
                let expected = format!("Bearer {}", token);
                // SEC-004: constant-time comparison to prevent timing attacks
                expected.as_bytes().ct_eq(header_str.as_bytes()).into()
            }
            None => false,
        };

        if authorized {
            let fut = self.inner.call(req);
            Box::pin(async move {
                let res = fut.await?;
                Ok(res)
            })
        } else {
            Box::pin(async move { Ok((StatusCode::UNAUTHORIZED, "Unauthorized").into_response()) })
        }
    }
}
