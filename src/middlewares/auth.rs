use axum::{
    body::{self, BoxBody},
    http::StatusCode,
    response::{IntoResponse, Response},
    BoxError, Json,
};
use bytes::Bytes;
use futures::future::BoxFuture;
use http::{self, HeaderValue, Request};
use http_body::Body as HttpBody;
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::{
    boxed::Box,
    convert::Infallible,
    task::{Context, Poll},
};
use tower::{Layer, Service};

use crate::AppState;

const AUTHORIZATION: &str = "Authorization";

const EMPTY: &str = "";

#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseBody<T> {
    pub message: String,
    pub data: T,
}

impl<T> ResponseBody<T> {
    pub fn new(message: &str, data: T) -> ResponseBody<T> {
        ResponseBody {
            message: message.to_string(),
            data,
        }
    }
}

#[derive(Clone)]
pub struct AuthLayer {
    state: AppState,
}

impl AuthLayer {
    pub fn new_with_state(state: AppState) -> AuthLayer {
        AuthLayer { state }
    }
}

impl<S> Layer<S> for AuthLayer {
    type Service = AuthMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthMiddleware {
            inner,
            state: self.state.clone(),
        }
    }
}

#[derive(Clone)]
pub struct AuthMiddleware<S> {
    inner: S,
    state: AppState,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for AuthMiddleware<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>, Error = Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    ReqBody: Send + 'static,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
{
    type Response = Response<BoxBody>;
    type Error = Infallible;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let not_ready_inner = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, not_ready_inner);

        match extract_token_from_header(req.headers().get(AUTHORIZATION)) {
            Ok(token) => {
                if token == self.state.config.openai_api_key {
                    info!("Valid token");
                    Box::pin(async move { Ok(inner.call(req).await?.map(body::boxed)) })
                } else {
                    error!("Invalid token");
                    Box::pin(async move { unauthorized(AuthError::InvalidToken) })
                }
            }
            Err(err) => Box::pin(async move { unauthorized(err) }),
        }
    }
}

enum AuthError {
    MissingAuthHeader,
    InvalidAuthHeader,
    InvalidToken,
}

impl AuthError {
    fn message(&self) -> &'static str {
        match self {
            AuthError::MissingAuthHeader => "Missing authorization header",
            AuthError::InvalidAuthHeader => "Invalid authorization header",
            AuthError::InvalidToken => "Invalid openai api key",
        }
    }

    fn status_code(&self) -> StatusCode {
        match self {
            AuthError::InvalidAuthHeader => StatusCode::UNPROCESSABLE_ENTITY,
            AuthError::MissingAuthHeader | AuthError::InvalidToken => StatusCode::UNAUTHORIZED,
        }
    }
}

fn extract_token_from_header(header: Option<&HeaderValue>) -> Result<String, AuthError> {
    match header {
        Some(header) => {
            if let Ok(auth_str) = header.to_str() {
                let parts: Vec<&str> = auth_str.split_whitespace().collect();
                if parts.len() == 2 && parts[0].eq_ignore_ascii_case("bearer") {
                    Ok(parts[1].to_string())
                } else {
                    Err(AuthError::InvalidAuthHeader)
                }
            } else {
                Err(AuthError::InvalidAuthHeader)
            }
        }
        None => Err(AuthError::MissingAuthHeader),
    }
}

fn unauthorized(error: AuthError) -> Result<Response<BoxBody>, Infallible> {
    Ok((
        error.status_code(),
        Json(ResponseBody::new(error.message(), EMPTY)),
    )
        .into_response())
}

#[cfg(test)]

mod tests {
    use super::*;
    use futures::future::{self, Ready};
    use http::header::HeaderValue;
    use hyper::{Body, Request};
    use tokio::runtime::Runtime;

    #[derive(Clone)]
    pub struct MockService;

    impl MockService {
        pub fn new() -> Self {
            Self
        }
    }

    impl Service<Request<Body>> for MockService {
        type Response = Response<Body>;
        type Error = Infallible;
        type Future = Ready<Result<Self::Response, Self::Error>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _req: Request<Body>) -> Self::Future {
            let body = Body::empty();
            let mut response = Response::new(body);
            response
                .headers_mut()
                .insert("content-type", HeaderValue::from_static("text/plain"));
            response
                .headers_mut()
                .insert("server", HeaderValue::from_static("MyService"));
            future::ok(response)
        }
    }

    fn mock_state() -> AppState {
        use crate::{api_client::ApiClient, config::AppConfig};

        let mock_client = ApiClient::new("http://example.com/v1".into(), "test-api-key".into());

        AppState {
            client: mock_client,
            config: AppConfig {
                openai_api_key: "test-api-key".to_string(),
                ..Default::default()
            },
        }
    }

    #[test]
    fn test_call_middleware() {
        let rt = Runtime::new().unwrap();

        let mut middleware = AuthMiddleware::<MockService> {
            inner: MockService::new(),
            state: mock_state(),
        };

        // Scenario 1: Request without authorization header
        let req_no_auth = Request::builder()
            .method("GET")
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        let resp_no_auth = rt.block_on(middleware.call(req_no_auth)).unwrap();
        assert_eq!(resp_no_auth.status(), 401); // Assuming MISSING_AUTHORIZATION_HEADER results in a 401 status code

        // Scenario 2: Request with invalid authorization header
        let req_invalid_auth = Request::builder()
            .method("GET")
            .uri("/test")
            .header(AUTHORIZATION, "InvalidAuth")
            .body(Body::empty())
            .unwrap();

        let resp_invalid_auth = rt.block_on(middleware.call(req_invalid_auth)).unwrap();
        assert_eq!(resp_invalid_auth.status(), 422); // Assuming INVALID_AUTHORIZATION_HEADER results in a 401 status code

        // Scenario 3: Request with invalid token
        let req_invalid_token = Request::builder()
            .method("GET")
            .uri("/test")
            .header(AUTHORIZATION, "bearer invalid_token")
            .body(Body::empty())
            .unwrap();

        let resp_invalid_token = rt.block_on(middleware.call(req_invalid_token)).unwrap();
        assert_eq!(resp_invalid_token.status(), 401); // Assuming MESSAGE_INVALID_OPENAI_API_KEY results in a 401 status code

        // Scenario 4: Request with valid token
        let req_valid_token = Request::builder()
            .method("GET")
            .uri("/test")
            .header(
                AUTHORIZATION,
                format!("bearer {}", middleware.state.config.openai_api_key),
            )
            .body(Body::empty())
            .unwrap();

        let resp_valid_token = rt.block_on(middleware.call(req_valid_token)).unwrap();
        assert_eq!(resp_valid_token.status(), 200); // Assuming a successful request results in a 200 status code
    }
}
