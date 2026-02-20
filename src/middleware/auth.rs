use axum::{
    body::Body,
    http::{Request, Response, StatusCode},

    response::IntoResponse,
};
use std::env;
use tower::{Layer, Service};
use std::task::{Context, Poll};
use std::future::Future;
use std::pin::Pin;

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
        // Skip auth for health check
        if req.uri().path() == "/health" {
            let fut = self.inner.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res)
            });
        }

        let token = env::var("API_TOKEN").unwrap_or_default();
        let auth_header = req.headers().get("Authorization");

        match auth_header {
            Some(header) => {
                let header_str = header.to_str().unwrap_or("");
                if header_str == format!("Bearer {}", token) {
                    let fut = self.inner.call(req);
                    return Box::pin(async move {
                        let res = fut.await?;
                        Ok(res)
                    });
                }
            }
            None => {}
        }

        Box::pin(async move {
            Ok((StatusCode::UNAUTHORIZED, "Unauthorized").into_response())
        })
    }
}
