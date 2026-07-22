use std::time::Duration;

use anyhow::Context as _;
use axum::extract::rejection::JsonRejection;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use tower_http::set_header::SetResponseHeaderLayer;

use crate::web_config::WebConfig;
use crate::web_types::{
    ApiErrorBody, ApiErrorCode, ApiErrorEnvelope, RegisterRuntimeRequest, RegisterRuntimeResponse,
    RuntimeAccount,
};
use crate::{OPENAI_AGENT_REGISTRATION_BASE_URL, parse_account_claims};

const REQUEST_BODY_LIMIT: usize = 32 * 1024;
const UPSTREAM_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Clone)]
pub struct WebState {
    pub client: reqwest::Client,
    pub registration_base_url: String,
}

impl WebState {
    pub fn from_config(config: &WebConfig) -> anyhow::Result<Self> {
        let builder = reqwest::Client::builder()
            .https_only(true)
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("agentidentity/", env!("CARGO_PKG_VERSION")));
        let client = config
            .apply_proxy(builder)?
            .build()
            .context("failed to initialize HTTPS registration client")?;

        Ok(Self::new(client, OPENAI_AGENT_REGISTRATION_BASE_URL))
    }

    pub fn new(client: reqwest::Client, registration_base_url: impl Into<String>) -> Self {
        Self {
            client,
            registration_base_url: registration_base_url.into(),
        }
    }
}

pub fn api_router(state: WebState) -> Router {
    Router::new()
        .route("/api/agent-runtimes", post(register_runtime))
        .layer(DefaultBodyLimit::max(REQUEST_BODY_LIMIT))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
        .with_state(state)
}

pub async fn register_runtime(
    State(state): State<WebState>,
    payload: Result<Json<RegisterRuntimeRequest>, JsonRejection>,
) -> Response {
    let Json(request) = match payload {
        Ok(request) => request,
        Err(_) => return ApiError::invalid_request().into_response(),
    };

    match register_runtime_inner(&state, request).await {
        Ok(response) => Json(response).into_response(),
        Err(error) => error.into_response(),
    }
}

async fn register_runtime_inner(
    state: &WebState,
    request: RegisterRuntimeRequest,
) -> Result<RegisterRuntimeResponse, ApiError> {
    validate_ed25519_public_key(&request.agent_public_key)?;

    let claims = parse_account_claims(&request.access_token).map_err(|error| {
        if error.to_string().contains("token has expired") {
            ApiError::token_expired()
        } else {
            ApiError::invalid_token()
        }
    })?;

    let url = reqwest::Url::parse(&format!(
        "{}/v1/agent/register",
        state.registration_base_url.trim_end_matches('/')
    ))
    .map_err(|_| ApiError::internal_error())?;
    let upstream_request = UpstreamRegisterRequest {
        abom: AgentBillOfMaterials {
            agent_version: env!("CARGO_PKG_VERSION"),
            agent_harness_id: "agentidentity",
            running_location: "web",
        },
        agent_public_key: &request.agent_public_key,
        capabilities: ["responsesapi"],
        ttl: None,
    };
    let mut request_builder = state
        .client
        .post(url)
        .bearer_auth(&request.access_token)
        .json(&upstream_request)
        .timeout(UPSTREAM_TIMEOUT);
    if claims.is_fedramp {
        request_builder = request_builder.header("X-OpenAI-Fedramp", "true");
    }

    let upstream_response = request_builder
        .send()
        .await
        .map_err(|_| ApiError::upstream_unavailable())?;
    let status = upstream_response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(ApiError::invalid_token());
    }
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(ApiError::upstream_unavailable());
    }
    if status.is_client_error() {
        return Err(ApiError::registration_rejected());
    }
    if !status.is_success() {
        return Err(ApiError::upstream_unavailable());
    }

    let upstream_response: UpstreamRegisterResponse = upstream_response
        .json()
        .await
        .map_err(|_| ApiError::upstream_unavailable())?;
    if upstream_response.agent_runtime_id.trim().is_empty() {
        return Err(ApiError::upstream_unavailable());
    }

    Ok(RegisterRuntimeResponse {
        agent_runtime_id: upstream_response.agent_runtime_id,
        account: RuntimeAccount {
            account_id: claims.account_id,
            chatgpt_user_id: claims.chatgpt_user_id,
            email: claims.email,
            plan_type: claims.plan_type,
        },
    })
}

fn validate_ed25519_public_key(public_key: &str) -> Result<(), ApiError> {
    let Some((algorithm, encoded_blob)) = public_key.split_once(' ') else {
        return Err(ApiError::invalid_request());
    };
    if algorithm != "ssh-ed25519"
        || encoded_blob.is_empty()
        || encoded_blob.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return Err(ApiError::invalid_request());
    }

    let blob = STANDARD
        .decode(encoded_blob)
        .map_err(|_| ApiError::invalid_request())?;
    let mut remaining = blob.as_slice();
    let blob_algorithm = take_ssh_string(&mut remaining).ok_or_else(ApiError::invalid_request)?;
    let key = take_ssh_string(&mut remaining).ok_or_else(ApiError::invalid_request)?;
    if blob_algorithm != b"ssh-ed25519" || key.len() != 32 || !remaining.is_empty() {
        return Err(ApiError::invalid_request());
    }

    Ok(())
}

fn take_ssh_string<'a>(input: &mut &'a [u8]) -> Option<&'a [u8]> {
    let length_bytes: [u8; 4] = input.get(..4)?.try_into().ok()?;
    let length = u32::from_be_bytes(length_bytes) as usize;
    let value = input.get(4..4_usize.checked_add(length)?)?;
    *input = input.get(4 + length..)?;
    Some(value)
}

#[derive(Serialize)]
struct UpstreamRegisterRequest<'a> {
    abom: AgentBillOfMaterials,
    agent_public_key: &'a str,
    capabilities: [&'static str; 1],
    ttl: Option<u64>,
}

#[derive(Serialize)]
struct AgentBillOfMaterials {
    agent_version: &'static str,
    agent_harness_id: &'static str,
    running_location: &'static str,
}

#[derive(Deserialize)]
struct UpstreamRegisterResponse {
    agent_runtime_id: String,
}

struct ApiError {
    status: StatusCode,
    code: ApiErrorCode,
    message: &'static str,
}

impl ApiError {
    fn invalid_request() -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: ApiErrorCode::InvalidRequest,
            message: "The request is invalid.",
        }
    }

    fn invalid_token() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: ApiErrorCode::InvalidToken,
            message: "The access token is invalid.",
        }
    }

    fn token_expired() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: ApiErrorCode::TokenExpired,
            message: "The access token has expired.",
        }
    }

    fn registration_rejected() -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: ApiErrorCode::RegistrationRejected,
            message: "OpenAI rejected the Agent Runtime registration.",
        }
    }

    fn upstream_unavailable() -> Self {
        Self {
            status: StatusCode::BAD_GATEWAY,
            code: ApiErrorCode::UpstreamUnavailable,
            message: "The OpenAI registration service is unavailable.",
        }
    }

    fn internal_error() -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: ApiErrorCode::InternalError,
            message: "The server could not complete the request.",
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ApiErrorEnvelope {
                error: ApiErrorBody {
                    code: self.code,
                    message: self.message.to_string(),
                },
            }),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read as _, Write as _};
    use std::net::TcpListener;
    use std::thread;

    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use serde_json::{Value, json};
    use tower::ServiceExt as _;

    use super::*;

    #[tokio::test]
    async fn valid_request_returns_account_and_sends_official_upstream_shape() {
        let (base_url, upstream) = fake_upstream(
            "200 OK",
            "application/json",
            r#"{"agent_runtime_id":"runtime-789"}"#,
        );
        let token = token_with_expiration(4_102_444_800);
        let public_key = valid_public_key();

        let response = send_api_request(
            &base_url,
            json!({
                "access_token": token,
                "agent_public_key": public_key,
            }),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let body = response_json(response).await;
        assert_eq!(
            body,
            json!({
                "agent_runtime_id": "runtime-789",
                "account": {
                    "account_id": "account-123",
                    "chatgpt_user_id": "user-456",
                    "email": "person@example.com",
                    "plan_type": "plus"
                }
            })
        );

        let request = upstream.join().unwrap();
        let (headers, body) = request.split_once("\r\n\r\n").unwrap();
        let body: Value = serde_json::from_str(body).unwrap();
        assert!(headers.starts_with("POST /v1/agent/register HTTP/1.1\r\n"));
        assert!(
            headers
                .to_ascii_lowercase()
                .contains("authorization: bearer ")
        );
        assert!(
            headers
                .to_ascii_lowercase()
                .contains("x-openai-fedramp: true\r\n")
        );
        assert_eq!(body["agent_public_key"], public_key);
        assert_eq!(body["abom"]["agent_harness_id"], "agentidentity");
        assert_eq!(body["abom"]["agent_version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(body["abom"]["running_location"], "web");
        assert_eq!(body["capabilities"], json!(["responsesapi"]));
        assert!(body["ttl"].is_null());
    }

    #[tokio::test]
    async fn expired_token_maps_to_token_expired_without_echoing_token() {
        let token = token_with_expiration(1);
        let response = send_api_request(
            "http://127.0.0.1:1",
            json!({
                "access_token": token,
                "agent_public_key": valid_public_key(),
            }),
        )
        .await;

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body = response_json(response).await;
        assert_eq!(body["error"]["code"], "TOKEN_EXPIRED");
        assert!(!body.to_string().contains(&token));
    }

    #[tokio::test]
    async fn malformed_json_maps_to_structured_invalid_request() {
        let response = api_router(WebState::new(test_client(), "http://127.0.0.1:1"))
            .oneshot(
                Request::post("/api/agent-runtimes")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{not-json"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response_json(response).await,
            json!({
                "error": {
                    "code": "INVALID_REQUEST",
                    "message": "The request is invalid."
                }
            })
        );
    }

    #[tokio::test]
    async fn invalid_public_key_maps_to_invalid_request() {
        let response = send_api_request(
            "http://127.0.0.1:1",
            json!({
                "access_token": token_with_expiration(4_102_444_800),
                "agent_public_key": "ssh-ed25519 AAAA",
            }),
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response_json(response).await,
            json!({
                "error": {
                    "code": "INVALID_REQUEST",
                    "message": "The request is invalid."
                }
            })
        );
    }

    #[tokio::test]
    async fn invalid_server_registration_url_maps_to_internal_error() {
        let response = send_api_request(
            "not a URL",
            json!({
                "access_token": token_with_expiration(4_102_444_800),
                "agent_public_key": valid_public_key(),
            }),
        )
        .await;

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response_json(response).await["error"]["code"],
            "INTERNAL_ERROR"
        );
    }

    #[tokio::test]
    async fn upstream_unauthorized_maps_to_invalid_token() {
        let (base_url, upstream) = fake_upstream("401 Unauthorized", "text/plain", "rejected");
        let response = send_api_request(
            &base_url,
            json!({
                "access_token": token_with_expiration(4_102_444_800),
                "agent_public_key": valid_public_key(),
            }),
        )
        .await;

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response_json(response).await["error"]["code"],
            "INVALID_TOKEN"
        );
        upstream.join().unwrap();
    }

    #[tokio::test]
    async fn upstream_rate_limit_maps_to_upstream_unavailable() {
        let (base_url, upstream) =
            fake_upstream("429 Too Many Requests", "text/plain", "slow down");
        let response = send_api_request(
            &base_url,
            json!({
                "access_token": token_with_expiration(4_102_444_800),
                "agent_public_key": valid_public_key(),
            }),
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(
            response_json(response).await["error"]["code"],
            "UPSTREAM_UNAVAILABLE"
        );
        upstream.join().unwrap();
    }

    #[tokio::test]
    async fn upstream_response_body_is_not_exposed() {
        let secret = "upstream-secret-that-must-not-leak";
        let (base_url, upstream) = fake_upstream("500 Internal Server Error", "text/plain", secret);
        let response = send_api_request(
            &base_url,
            json!({
                "access_token": token_with_expiration(4_102_444_800),
                "agent_public_key": valid_public_key(),
            }),
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        let body = response_json(response).await;
        assert_eq!(body["error"]["code"], "UPSTREAM_UNAVAILABLE");
        assert!(!body.to_string().contains(secret));
        upstream.join().unwrap();
    }

    fn test_client() -> reqwest::Client {
        reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap()
    }

    async fn send_api_request(base_url: &str, body: Value) -> Response {
        api_router(WebState::new(test_client(), base_url))
            .oneshot(
                Request::post("/api/agent-runtimes")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn response_json(response: Response) -> Value {
        let bytes = to_bytes(response.into_body(), REQUEST_BODY_LIMIT)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn token_with_expiration(expiration: i64) -> String {
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
        let claims = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&json!({
                "exp": expiration,
                "email": "person@example.com",
                "https://api.openai.com/auth": {
                    "chatgpt_account_id": "account-123",
                    "chatgpt_user_id": "user-456",
                    "chatgpt_plan_type": "plus",
                    "chatgpt_account_is_fedramp": true
                }
            }))
            .unwrap(),
        );
        let signature = URL_SAFE_NO_PAD.encode(b"signature");
        format!("{header}.{claims}.{signature}")
    }

    fn valid_public_key() -> String {
        let mut blob = Vec::new();
        append_ssh_string(&mut blob, b"ssh-ed25519");
        append_ssh_string(&mut blob, &[7; 32]);
        format!("ssh-ed25519 {}", STANDARD.encode(blob))
    }

    fn append_ssh_string(output: &mut Vec<u8>, value: &[u8]) {
        output.extend_from_slice(&(value.len() as u32).to_be_bytes());
        output.extend_from_slice(value);
    }

    fn fake_upstream(
        status: &'static str,
        content_type: &'static str,
        response_body: &'static str,
    ) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 4096];
            loop {
                let read = stream.read(&mut buffer).unwrap();
                request.extend_from_slice(&buffer[..read]);
                let Some(header_end) = request.windows(4).position(|part| part == b"\r\n\r\n")
                else {
                    continue;
                };
                let headers = String::from_utf8_lossy(&request[..header_end + 4]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .map(str::to_owned)
                    })
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                if request.len() >= header_end + 4 + content_length {
                    break;
                }
            }

            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}",
                response_body.len()
            )
            .unwrap();
            String::from_utf8(request).unwrap()
        });

        (format!("http://{address}"), server)
    }
}
