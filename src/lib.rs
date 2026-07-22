use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::Utc;
use ed25519_dalek::SigningKey;
use ed25519_dalek::pkcs8::EncodePrivateKey;
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

const OPENAI_AGENT_REGISTRATION_BASE_URL: &str = "https://auth.openai.com/api/accounts";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountClaims {
    pub account_id: String,
    pub chatgpt_user_id: String,
    pub email: String,
    pub plan_type: String,
    pub is_fedramp: bool,
}

#[derive(Deserialize)]
struct JwtClaims {
    exp: i64,
    #[serde(default)]
    email: Option<String>,
    #[serde(rename = "https://api.openai.com/profile", default)]
    profile: Option<ProfileClaims>,
    #[serde(rename = "https://api.openai.com/auth")]
    auth: AuthClaims,
}

#[derive(Deserialize)]
struct ProfileClaims {
    #[serde(default)]
    email: Option<String>,
}

#[derive(Deserialize)]
struct AuthClaims {
    chatgpt_account_id: String,
    #[serde(default)]
    chatgpt_user_id: Option<String>,
    #[serde(default)]
    user_id: Option<String>,
    chatgpt_plan_type: String,
    #[serde(default)]
    chatgpt_account_is_fedramp: bool,
}

pub struct GeneratedKeyMaterial {
    private_key_pkcs8_base64: Zeroizing<String>,
    public_key_ssh: String,
}

#[derive(Serialize)]
struct RegisterAgentRequest<'a> {
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
struct RegisterAgentResponse {
    agent_runtime_id: String,
}

#[derive(Serialize)]
pub struct IdentityDocument<'a> {
    auth_mode: &'static str,
    agent_identity: IdentityDetails<'a>,
}

#[derive(Serialize)]
struct IdentityDetails<'a> {
    agent_runtime_id: &'a str,
    agent_private_key: &'a str,
    account_id: &'a str,
    chatgpt_user_id: &'a str,
    email: &'a str,
    plan_type: &'a str,
}

pub fn generate_key_material() -> Result<GeneratedKeyMaterial> {
    let signing_key = SigningKey::generate(&mut OsRng);
    let private_key = signing_key
        .to_pkcs8_der()
        .context("failed to encode Ed25519 private key as PKCS#8")?;

    let mut public_key_blob = Vec::with_capacity(51);
    append_ssh_string(&mut public_key_blob, b"ssh-ed25519");
    append_ssh_string(&mut public_key_blob, signing_key.verifying_key().as_bytes());

    Ok(GeneratedKeyMaterial {
        private_key_pkcs8_base64: Zeroizing::new(
            base64::engine::general_purpose::STANDARD.encode(private_key.as_bytes()),
        ),
        public_key_ssh: format!(
            "ssh-ed25519 {}",
            base64::engine::general_purpose::STANDARD.encode(public_key_blob)
        ),
    })
}

fn append_ssh_string(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u32).to_be_bytes());
    output.extend_from_slice(value);
}

pub fn build_registration_client() -> Result<reqwest::blocking::Client> {
    build_registration_client_with_https_only(true)
}

fn build_registration_client_with_https_only(
    https_only: bool,
) -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .https_only(https_only)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("agentidentity/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("failed to initialize HTTPS client")
}

pub fn register_with_openai(
    client: &reqwest::blocking::Client,
    access_token: &str,
    is_fedramp: bool,
    key_material: &GeneratedKeyMaterial,
) -> Result<String> {
    register_agent_identity(
        client,
        OPENAI_AGENT_REGISTRATION_BASE_URL,
        access_token,
        is_fedramp,
        key_material,
    )
}

fn register_agent_identity(
    client: &reqwest::blocking::Client,
    base_url: &str,
    access_token: &str,
    is_fedramp: bool,
    key_material: &GeneratedKeyMaterial,
) -> Result<String> {
    let url = format!("{}/v1/agent/register", base_url.trim_end_matches('/'));
    let request = RegisterAgentRequest {
        abom: AgentBillOfMaterials {
            agent_version: env!("CARGO_PKG_VERSION"),
            agent_harness_id: "agentidentity",
            running_location: "cli-linux",
        },
        agent_public_key: &key_material.public_key_ssh,
        capabilities: ["responsesapi"],
        ttl: None,
    };

    let mut request_builder = client
        .post(url)
        .bearer_auth(access_token)
        .json(&request)
        .timeout(std::time::Duration::from_secs(15));
    if is_fedramp {
        request_builder = request_builder.header("X-OpenAI-Fedramp", "true");
    }

    let response = request_builder
        .send()
        .context("failed to send Agent Identity registration request")?;
    ensure!(
        response.status().is_success(),
        "Agent Identity registration failed with HTTP {}",
        response.status()
    );
    let response: RegisterAgentResponse = response
        .json()
        .context("Agent Identity registration returned invalid JSON")?;
    ensure!(
        !response.agent_runtime_id.trim().is_empty(),
        "Agent Identity registration omitted agent_runtime_id"
    );
    Ok(response.agent_runtime_id)
}

pub fn build_identity_document<'a>(
    runtime_id: &'a str,
    key_material: &'a GeneratedKeyMaterial,
    claims: &'a AccountClaims,
) -> IdentityDocument<'a> {
    IdentityDocument {
        auth_mode: "agentIdentity",
        agent_identity: IdentityDetails {
            agent_runtime_id: runtime_id,
            agent_private_key: key_material.private_key_pkcs8_base64.as_str(),
            account_id: &claims.account_id,
            chatgpt_user_id: &claims.chatgpt_user_id,
            email: &claims.email,
            plan_type: &claims.plan_type,
        },
    }
}

pub fn write_identity_file(path: &Path, document: &impl Serialize) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("output filename is not valid UTF-8")?;
    let temporary_path = unique_temporary_path(parent, file_name);

    let result = (|| -> Result<()> {
        let mut temporary_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
            .with_context(|| {
                format!(
                    "failed to create temporary output file in {}",
                    parent.display()
                )
            })?;
        serde_json::to_writer_pretty(&mut temporary_file, document)
            .context("failed to serialize Agent Identity JSON")?;
        temporary_file
            .write_all(b"\n")
            .context("failed to finish Agent Identity JSON")?;
        temporary_file
            .sync_all()
            .context("failed to sync Agent Identity JSON")?;
        drop(temporary_file);
        std::fs::rename(&temporary_path, path)
            .with_context(|| format!("failed to replace {}", path.display()))?;
        if let Ok(directory) = File::open(parent) {
            let _ = directory.sync_all();
        }
        Ok(())
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&temporary_path);
    }
    result
}

fn unique_temporary_path(parent: &Path, file_name: &str) -> PathBuf {
    let nonce = rand_core::RngCore::next_u64(&mut OsRng);
    parent.join(format!(".{file_name}.{nonce:016x}.tmp"))
}

pub fn output_filename(email: &str, plan_type: &str) -> String {
    fn sanitize(value: &str) -> String {
        value
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                    character
                } else {
                    '_'
                }
            })
            .collect()
    }

    format!(
        "agent-identity-{}-{}.json",
        sanitize(email),
        sanitize(plan_type)
    )
}

pub fn parse_account_claims(token: &str) -> Result<AccountClaims> {
    let mut parts = token.split('.');
    let (Some(header), Some(payload), Some(signature)) = (parts.next(), parts.next(), parts.next())
    else {
        anyhow::bail!("token must be a three-part JWT");
    };
    ensure!(
        !header.is_empty()
            && !payload.is_empty()
            && !signature.is_empty()
            && parts.next().is_none(),
        "token must be a three-part JWT"
    );

    let header = URL_SAFE_NO_PAD
        .decode(header)
        .context("JWT header is not valid base64url")?;
    serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(&header)
        .context("JWT header is not valid JSON")?;
    let payload = URL_SAFE_NO_PAD
        .decode(payload)
        .context("JWT payload is not valid base64url")?;
    URL_SAFE_NO_PAD
        .decode(signature)
        .context("JWT signature is not valid base64url")?;
    let claims: JwtClaims =
        serde_json::from_slice(&payload).context("JWT payload is not valid JSON")?;
    ensure!(claims.exp > Utc::now().timestamp(), "token has expired");

    let email = claims
        .email
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            claims
                .profile
                .and_then(|profile| profile.email)
                .filter(|value| !value.trim().is_empty())
        })
        .context("token is missing email")?;
    let chatgpt_user_id = claims
        .auth
        .chatgpt_user_id
        .filter(|value| !value.trim().is_empty())
        .or_else(|| claims.auth.user_id.filter(|value| !value.trim().is_empty()))
        .context("token is missing chatgpt_user_id")?;
    ensure!(
        !claims.auth.chatgpt_account_id.trim().is_empty(),
        "token is missing account_id"
    );
    ensure!(
        !claims.auth.chatgpt_plan_type.trim().is_empty(),
        "token is missing plan_type"
    );

    Ok(AccountClaims {
        account_id: claims.auth.chatgpt_account_id,
        chatgpt_user_id,
        email,
        plan_type: claims.auth.chatgpt_plan_type,
        is_fedramp: claims.auth.chatgpt_account_is_fedramp,
    })
}

#[cfg(test)]
mod tests {
    use base64::Engine as _;
    use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use ed25519_dalek::SigningKey;
    use ed25519_dalek::pkcs8::DecodePrivateKey;
    use serde_json::json;

    use super::*;

    fn unsigned_token(payload: serde_json::Value) -> String {
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
        let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap());
        let signature = URL_SAFE_NO_PAD.encode(b"signature");
        format!("{header}.{payload}.{signature}")
    }

    #[test]
    fn builds_sanitized_account_filename() {
        assert_eq!(
            output_filename("person+work@example.com", "team / annual"),
            "agent-identity-person_work_example.com-team___annual.json"
        );
    }

    #[test]
    fn generated_identity_document_contains_reusable_key_and_account_metadata() {
        let claims = AccountClaims {
            account_id: "account-123".to_string(),
            chatgpt_user_id: "user-456".to_string(),
            email: "person@example.com".to_string(),
            plan_type: "plus".to_string(),
            is_fedramp: false,
        };

        let key_material = generate_key_material().unwrap();
        let document = build_identity_document("runtime-789", &key_material, &claims);
        let document = serde_json::to_value(document).unwrap();
        let encoded_private_key = document["agent_identity"]["agent_private_key"]
            .as_str()
            .unwrap();
        let private_key_der = STANDARD.decode(encoded_private_key).unwrap();

        SigningKey::from_pkcs8_der(&private_key_der).unwrap();
        assert_eq!(document["auth_mode"], "agentIdentity");
        assert_eq!(
            document["agent_identity"]["agent_runtime_id"],
            "runtime-789"
        );
        assert_eq!(document["agent_identity"]["account_id"], "account-123");
        assert_eq!(document["agent_identity"]["chatgpt_user_id"], "user-456");
        assert_eq!(document["agent_identity"]["email"], "person@example.com");
        assert_eq!(document["agent_identity"]["plan_type"], "plus");
        assert!(document["agent_identity"].get("task_id").is_none());
    }

    #[test]
    fn registers_runtime_with_expected_official_request_shape() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 4096];
            loop {
                let read = stream.read(&mut buffer).unwrap();
                request.extend_from_slice(&buffer[..read]);
                if let Some(header_end) = request.windows(4).position(|part| part == b"\r\n\r\n") {
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
            }
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 34\r\nConnection: close\r\n\r\n{\"agent_runtime_id\":\"runtime-789\"}",
                )
                .unwrap();
            String::from_utf8(request).unwrap()
        });
        let key_material = generate_key_material().unwrap();
        let client = reqwest::blocking::Client::new();

        let runtime_id = register_agent_identity(
            &client,
            &format!("http://{address}"),
            "secret-token",
            true,
            &key_material,
        )
        .unwrap();
        let request = server.join().unwrap();
        let (_, body) = request.split_once("\r\n\r\n").unwrap();
        let body: serde_json::Value = serde_json::from_str(body).unwrap();

        assert_eq!(runtime_id, "runtime-789");
        assert!(request.starts_with("POST /v1/agent/register HTTP/1.1\r\n"));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer secret-token\r\n")
        );
        assert!(
            request
                .to_ascii_lowercase()
                .contains("x-openai-fedramp: true\r\n")
        );
        assert_eq!(body["agent_public_key"], key_material.public_key_ssh);
        assert_eq!(body["abom"]["agent_harness_id"], "agentidentity");
        assert_eq!(body["abom"]["agent_version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(body["abom"]["running_location"], "cli-linux");
        assert_eq!(body["capabilities"], json!(["responsesapi"]));
        assert!(body["ttl"].is_null());
    }

    #[test]
    fn atomically_replaces_existing_identity_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("identity.json");
        std::fs::write(&path, "old data").unwrap();
        let document = json!({"auth_mode": "agentIdentity"});

        write_identity_file(&path, &document).unwrap();

        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "{\n  \"auth_mode\": \"agentIdentity\"\n}\n"
        );
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn parses_official_namespaced_account_claims() {
        let token = unsigned_token(json!({
            "exp": 4_102_444_800_i64,
            "email": "person@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "account-123",
                "chatgpt_user_id": "user-456",
                "chatgpt_plan_type": "plus",
                "chatgpt_account_is_fedramp": true
            }
        }));

        let claims = parse_account_claims(&token).unwrap();

        assert_eq!(claims.account_id, "account-123");
        assert_eq!(claims.chatgpt_user_id, "user-456");
        assert_eq!(claims.email, "person@example.com");
        assert_eq!(claims.plan_type, "plus");
        assert!(claims.is_fedramp);
    }

    #[test]
    fn falls_back_to_profile_email_and_legacy_user_id() {
        let token = unsigned_token(json!({
            "exp": 4_102_444_800_i64,
            "https://api.openai.com/profile": {
                "email": "profile@example.com"
            },
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "account-123",
                "user_id": "legacy-user-456",
                "chatgpt_plan_type": "free"
            }
        }));

        let claims = parse_account_claims(&token).unwrap();

        assert_eq!(claims.email, "profile@example.com");
        assert_eq!(claims.chatgpt_user_id, "legacy-user-456");
        assert!(!claims.is_fedramp);
    }

    #[test]
    fn falls_back_when_preferred_email_and_user_id_are_blank() {
        let token = unsigned_token(json!({
            "exp": 4_102_444_800_i64,
            "email": "   ",
            "https://api.openai.com/profile": {
                "email": "profile@example.com"
            },
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "account-123",
                "chatgpt_user_id": " ",
                "user_id": "legacy-user-456",
                "chatgpt_plan_type": "free"
            }
        }));

        let claims = parse_account_claims(&token).unwrap();

        assert_eq!(claims.email, "profile@example.com");
        assert_eq!(claims.chatgpt_user_id, "legacy-user-456");
    }

    #[test]
    fn rejects_invalid_jwt_header_or_signature_encoding() {
        let payload = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&json!({
                "exp": 4_102_444_800_i64,
                "email": "person@example.com",
                "https://api.openai.com/auth": {
                    "chatgpt_account_id": "account-123",
                    "chatgpt_user_id": "user-456",
                    "chatgpt_plan_type": "plus"
                }
            }))
            .unwrap(),
        );

        let invalid_header = format!("not+base64.{payload}.signature");
        assert!(
            parse_account_claims(&invalid_header)
                .unwrap_err()
                .to_string()
                .contains("JWT header is not valid base64url")
        );

        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
        let invalid_signature = format!("{header}.{payload}.not+base64");
        assert!(
            parse_account_claims(&invalid_signature)
                .unwrap_err()
                .to_string()
                .contains("JWT signature is not valid base64url")
        );
    }

    #[test]
    fn rejects_expired_token_without_echoing_it() {
        let token = unsigned_token(json!({
            "exp": 1,
            "email": "person@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "account-123",
                "chatgpt_user_id": "user-456",
                "chatgpt_plan_type": "plus"
            }
        }));

        let error = parse_account_claims(&token).unwrap_err().to_string();

        assert!(error.contains("token has expired"));
        assert!(!error.contains(&token));
    }

    #[test]
    fn rejects_missing_required_claims_without_echoing_token() {
        let token = unsigned_token(json!({
            "exp": 4_102_444_800_i64,
            "email": "person@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_account_id": "account-123",
                "chatgpt_plan_type": "plus"
            }
        }));

        let error = parse_account_claims(&token).unwrap_err().to_string();

        assert!(error.contains("token is missing chatgpt_user_id"));
        assert!(!error.contains(&token));
    }

    #[test]
    fn registration_client_does_not_follow_redirects() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 4096];
            let _ = stream.read(&mut buffer).unwrap();
            stream
                .write_all(
                    b"HTTP/1.1 307 Temporary Redirect\r\nLocation: http://127.0.0.1:1/redirected\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .unwrap();
        });
        let key_material = generate_key_material().unwrap();
        let client = build_registration_client_with_https_only(false).unwrap();

        let error = register_agent_identity(
            &client,
            &format!("http://{address}"),
            "client-secret-token",
            false,
            &key_material,
        )
        .unwrap_err()
        .to_string();
        server.join().unwrap();

        assert!(error.contains("HTTP 307 Temporary Redirect"));
    }

    #[test]
    fn registration_http_error_does_not_echo_response_or_token() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 4096];
            let _ = stream.read(&mut buffer).unwrap();
            let body = "server-secret-response";
            write!(
                stream,
                "HTTP/1.1 401 Unauthorized\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let key_material = generate_key_material().unwrap();
        let client = reqwest::blocking::Client::new();
        let token = "client-secret-token";

        let error = register_agent_identity(
            &client,
            &format!("http://{address}"),
            token,
            false,
            &key_material,
        )
        .unwrap_err()
        .to_string();
        server.join().unwrap();

        assert!(error.contains("HTTP 401 Unauthorized"));
        assert!(!error.contains("server-secret-response"));
        assert!(!error.contains(token));
    }
}
