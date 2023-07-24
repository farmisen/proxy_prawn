use axum::{
    body::{self, BoxBody},
    http::header::{HeaderName, HeaderValue},
    response::{IntoResponse, Response},
    BoxError, Json,
};
use bytes::Bytes;
use futures::future::BoxFuture;
use http::{self, Request};
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
const MESSAGE_INVALID_OPENAI_API_KEY: &str = "Invalid openai api key";
const MISSING_OPENAI_API_KEY: &str = "Missing openai api key";
const MISSING_AUTHORIZATION_HEADER: &str = "Missing authorization header";
const INVALID_AUTHORIZATION_HEADER: &str = "Invalid authorization header";

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
    Infallible: From<<S as Service<Request<ReqBody>>>::Error>,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<BoxError>,
{
    type Response = Response<BoxBody>;
    type Error = Infallible;
    // `BoxFuture` is a type alias for `Pin<Box<dyn Future + Send + 'a>>`
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
        let not_ready_inner = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, not_ready_inner);

        // Bypass account routes
        let headers = req.headers_mut();
        headers.append(
            HeaderName::from_static("content-length"),
            HeaderValue::from_static("true"),
        );

        if let Some(auth_header) = req.headers().get(AUTHORIZATION) {
            info!("Parsing authorization header...");
            if let Ok(auth_str) = auth_header.to_str() {
                if auth_str.starts_with("bearer") || auth_str.starts_with("Bearer") {
                    info!("Parsing token...");
                    let token = auth_str[6..auth_str.len()].trim();
                    info!("Decoding token...");
                    if token == self.state.config.openai_api_key {
                        info!("Valid token");
                        Box::pin(async move { Ok(inner.call(req).await?.map(body::boxed)) })
                    } else {
                        error!("Invalid token");
                        Box::pin(async move {
                            Ok(
                                Json(ResponseBody::new(MESSAGE_INVALID_OPENAI_API_KEY, EMPTY))
                                    .into_response(),
                            )
                        })
                    }
                } else {
                    Box::pin(async move {
                        Ok(Json(ResponseBody::new(MISSING_OPENAI_API_KEY, EMPTY)).into_response())
                    })
                }
            } else {
                Box::pin(async move {
                    Ok(
                        Json(ResponseBody::new(INVALID_AUTHORIZATION_HEADER, EMPTY))
                            .into_response(),
                    )
                })
            }
        } else {
            Box::pin(async move {
                Ok(Json(ResponseBody::new(MISSING_AUTHORIZATION_HEADER, EMPTY)).into_response())
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::Future;
    use tokio::runtime::Runtime;
    use mockall::*;
    use mockall::predicate::*;
    use http_body::Body as HttpBody;
    use std::pin::Pin;
    use axum::body::Body;
    use futures::future::{self, Ready};
    use http::{Request, Response};
    use std::task::{Context, Poll};
    use std::convert::Infallible;
    use hyper::Body as HyperBody;
    use tower::Service;


    #[derive(Debug, Clone)]
    pub struct MockService;

    impl Service<Request<Body>> for MockService {
        type Response = Response<HyperBody>;
        type Error = Infallible;
        type Future = Ready<Result<Self::Response, Self::Error>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _req: Request<Body>) -> Self::Future {
            future::ready(Ok(Response::new(HyperBody::empty())))
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
        let mut rt = Runtime::new().unwrap();

        let mut mock_inner_service = MockService{};

        // let mut layer = AuthLayer::new_with_state(mock_state());  // Initialize your middleware here
        let mut middleware = AuthMiddleware {
            inner: mock_inner_service.clone(),
            state: mock_state(),
        };


        // Scenario 1: Request without authorization header
        let req_no_auth = Request::builder()
            .method("GET")
            .uri("/test")
            .body(axum::body::Body::empty())
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
        assert_eq!(resp_invalid_auth.status(), 401); // Assuming INVALID_AUTHORIZATION_HEADER results in a 401 status code

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
