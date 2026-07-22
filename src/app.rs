use leptos::prelude::*;

#[cfg(feature = "hydrate")]
use crate::web_types::{ApiErrorEnvelope, RegisterRuntimeRequest, RegisterRuntimeResponse};

const ACCESS_TOKEN_URL: &str = "https://chatgpt.com/api/auth/session";
const GITHUB_URL: &str = "https://github.com/FactrueSolin/openaiagentidentity";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Language {
    En,
    Zh,
}

#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    GeneratingKey,
    PreparingRequest,
    Registering,
    Assembling,
    Ready,
    Error,
}

#[cfg_attr(not(feature = "hydrate"), allow(dead_code))]
#[derive(Clone, Copy, PartialEq, Eq)]
enum CopyState {
    Idle,
    Copied,
    Error,
}

#[derive(Clone, PartialEq, Eq)]
struct IdentityOutput {
    json: String,
    filename: String,
}

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta
                    name="viewport"
                    content="width=device-width, initial-scale=1, viewport-fit=cover"
                />
                <meta
                    name="description"
                    content="Generate an OpenAI Agent Identity with browser-held Ed25519 key material."
                />
                <AutoReload options=options.clone()/>
                <HydrationScripts options/>
                <style id="design-tokens">{include_str!("../tokens.css")}</style>
                <link rel="stylesheet" href="/pkg/agentidentity-web.css"/>
                <title>"Agent Identity · Web"</title>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    let (language, set_language) = signal(Language::En);
    let (token, set_token) = signal(String::new());
    let (show_token, set_show_token) = signal(false);
    let (phase, set_phase) = signal(Phase::Idle);
    let (error_code, set_error_code) = signal(None::<String>);
    let (failed_step, set_failed_step) = signal(0_usize);
    let (output, set_output) = signal(None::<IdentityOutput>);
    let (copy_state, set_copy_state) = signal(CopyState::Idle);

    Effect::new(move |_| {
        set_language.set(browser_language());
    });
    Effect::new(move |_| {
        set_document_language(language.get());
    });

    let toggle_language = move |_| {
        set_language.update(|current| {
            *current = match *current {
                Language::En => Language::Zh,
                Language::Zh => Language::En,
            };
        });
    };

    let submit = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        if is_processing(phase.get_untracked()) {
            return;
        }

        let access_token = token.get_untracked().trim().to_owned();
        if access_token.is_empty() {
            set_error_code.set(Some("TOKEN_REQUIRED".to_owned()));
            set_failed_step.set(0);
            set_phase.set(Phase::Error);
            return;
        }

        set_error_code.set(None);
        set_failed_step.set(0);
        set_output.set(None);
        set_copy_state.set(CopyState::Idle);
        set_phase.set(Phase::GeneratingKey);

        #[cfg(feature = "hydrate")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let result = generate_identity(access_token, set_phase).await;
                match result {
                    Ok(identity_output) => {
                        set_phase.set(Phase::Ready);
                        set_token.set(String::new());
                        set_output.set(Some(identity_output));
                    }
                    Err(code) => {
                        set_error_code.set(Some(code));
                        set_failed_step.set(phase_step(phase.get_untracked()));
                        set_phase.set(Phase::Error);
                    }
                }
            });
        }
    };

    let copy_output = move |_| {
        let Some(identity_output) = output.get_untracked() else {
            return;
        };
        set_copy_state.set(CopyState::Idle);

        start_copy(identity_output, copy_state, set_copy_state);
    };

    let download_output = move |_| {
        let Some(identity_output) = output.get_untracked() else {
            return;
        };
        set_error_code.set(None);

        if start_download(&identity_output).is_err() {
            set_error_code.set(Some("DOWNLOAD_FAILED".to_owned()));
        }
    };

    view! {
        <header class="topbar">
            <a class="wordmark" href="#studio" aria-label="Agent Identity Web">
                "AGENT IDENTITY · WEB"
            </a>
            <nav class="topbar__nav" aria-label=move || text(language.get(), "Site", "站点")>
                <a class="nav-link" href=GITHUB_URL target="_blank" rel="noreferrer">
                    "GitHub"
                </a>
                <button
                    class="language-toggle"
                    type="button"
                    on:click=toggle_language
                    aria-label=move || {
                        text(language.get(), "Switch language", "切换语言")
                    }
                >
                    {move || match language.get() {
                        Language::En => "中文",
                        Language::Zh => "EN",
                    }}
                </button>
            </nav>
        </header>

        <main>
            <section id="studio" class="studio" aria-labelledby="studio-title">
                <div class="operation-pane">
                    <div class="operation-pane__intro">
                        <p class="kicker">"OPENAI · ED25519"</p>
                        <h1 id="studio-title">
                            {move || {
                                text(
                                    language.get(),
                                    "Generate your Agent Identity.",
                                    "生成你的 Agent Identity。",
                                )
                            }}
                        </h1>
                        <p class="lede">
                            {move || {
                                text(
                                    language.get(),
                                    "Create key material locally, register the public key, and assemble the reusable identity document in this browser.",
                                    "在本地创建密钥材料，注册公钥，并在当前浏览器中组装可复用的身份文档。",
                                )
                            }}
                        </p>
                    </div>

                    <form class="identity-form" on:submit=submit novalidate>
                        <div class="field">
                            <div class="field__label-row">
                                <label for="access-token">
                                    {move || text(language.get(), "Access Token", "Access Token")}
                                </label>
                                <button
                                    class="visibility-toggle"
                                    type="button"
                                    on:click=move |_| set_show_token.update(|value| *value = !*value)
                                    aria-controls="access-token"
                                    aria-pressed=move || show_token.get()
                                >
                                    {move || match (language.get(), show_token.get()) {
                                        (Language::En, false) => "Show",
                                        (Language::En, true) => "Hide",
                                        (Language::Zh, false) => "显示",
                                        (Language::Zh, true) => "隐藏",
                                    }}
                                </button>
                            </div>
                            <input
                                id="access-token"
                                class="token-input"
                                class:token-input--error=move || {
                                    phase.get() == Phase::Error
                                        && matches!(
                                            error_code.get().as_deref(),
                                            Some("TOKEN_REQUIRED" | "INVALID_TOKEN" | "TOKEN_EXPIRED")
                                        )
                                }
                                type=move || if show_token.get() { "text" } else { "password" }
                                autocomplete="off"
                                spellcheck="false"
                                aria-required="true"
                                aria-invalid=move || {
                                    phase.get() == Phase::Error
                                        && matches!(
                                            error_code.get().as_deref(),
                                            Some("TOKEN_REQUIRED" | "INVALID_TOKEN" | "TOKEN_EXPIRED")
                                        )
                                }
                                aria-describedby="token-help"
                                prop:value=move || token.get()
                                on:input=move |event| {
                                    set_token.set(event_target_value(&event));
                                    if error_code.get_untracked().as_deref() == Some("TOKEN_REQUIRED") {
                                        set_error_code.set(None);
                                        set_phase.set(Phase::Idle);
                                    }
                                }
                            />
                            <div id="token-help" class="field__help">
                                {move || {
                                    if phase.get() == Phase::Error
                                        && let Some(code) = error_code.get()
                                    {
                                        return localized_error(language.get(), &code);
                                    }
                                    text(
                                        language.get(),
                                        "Visible only in this tab's memory. It is cleared after success.",
                                        "仅存在于当前标签页内存中，成功后即清除。",
                                    )
                                }}
                            </div>
                        </div>

                        <div class="form-actions">
                            <button
                                class="primary-action"
                                class:primary-action--loading=move || is_processing(phase.get())
                                type="submit"
                                disabled=move || is_processing(phase.get())
                                aria-disabled=move || is_processing(phase.get())
                            >
                                <span class="primary-action__indicator" aria-hidden="true"></span>
                                <span>
                                    {move || {
                                        if is_processing(phase.get()) {
                                            text(language.get(), "Processing", "处理中")
                                        } else {
                                            text(language.get(), "Generate identity", "生成身份")
                                        }
                                    }}
                                </span>
                            </button>
                            <a
                                class="token-link"
                                href=ACCESS_TOKEN_URL
                                target="_blank"
                                rel="noreferrer"
                            >
                                {move || text(language.get(), "Obtain token", "获取 Token")}
                            </a>
                        </div>
                    </form>

                    <ol class="progress" aria-live="polite" aria-atomic="true">
                        {move || {
                            (0..4)
                                .map(|index| {
                                    let state = step_state(phase.get(), index, failed_step.get());
                                    view! {
                                        <li class=format!("progress__step progress__step--{state}")>
                                            <span class="progress__index">{format!("0{}", index + 1)}</span>
                                            <span>{step_label(language.get(), index)}</span>
                                        </li>
                                    }
                                })
                                .collect_view()
                        }}
                    </ol>
                </div>

                <div class="result-pane">
                    {move || {
                        if let Some(identity_output) = output.get() {
                            view! {
                                <div class="result-pane__content result-pane__content--ready">
                                    <div class="result-header">
                                        <div>
                                            <p class="result-status">
                                                {text(language.get(), "IDENTITY READY", "身份已就绪")}
                                            </p>
                                            <h2>{text(language.get(), "Browser-assembled JSON", "浏览器组装的 JSON")}</h2>
                                        </div>
                                        <div class="result-actions">
                                            <button
                                                class="secondary-action"
                                                type="button"
                                                on:click=copy_output
                                                data-state=move || match copy_state.get() {
                                                    CopyState::Idle => "idle",
                                                    CopyState::Copied => "success",
                                                    CopyState::Error => "error",
                                                }
                                            >
                                                {move || match (language.get(), copy_state.get()) {
                                                    (Language::En, CopyState::Idle) => "Copy",
                                                    (Language::En, CopyState::Copied) => "Copied",
                                                    (Language::En, CopyState::Error) => "Copy failed",
                                                    (Language::Zh, CopyState::Idle) => "复制",
                                                    (Language::Zh, CopyState::Copied) => "已复制",
                                                    (Language::Zh, CopyState::Error) => "复制失败",
                                                }}
                                            </button>
                                            <button
                                                class="secondary-action secondary-action--accent"
                                                type="button"
                                                on:click=download_output
                                            >
                                                {text(language.get(), "Download", "下载")}
                                            </button>
                                        </div>
                                    </div>
                                    <pre class="identity-json" tabindex="0"><code>{identity_output.json}</code></pre>
                                    <p class="filename">{identity_output.filename}</p>
                                    <p class="result-error" role="alert">
                                        {move || {
                                            error_code
                                                .get()
                                                .as_deref()
                                                .filter(|code| *code == "DOWNLOAD_FAILED")
                                                .map(|code| localized_error(language.get(), code))
                                                .unwrap_or_default()
                                        }}
                                    </p>
                                </div>
                            }
                                .into_any()
                        } else {
                            view! {
                                <div class="result-pane__content">
                                    <div class="boundary-heading">
                                        <p class="result-status">"DATA BOUNDARY"</p>
                                        <h2>{text(language.get(), "What crosses the boundary", "哪些数据会跨越边界")}</h2>
                                        <p>
                                            {text(
                                                language.get(),
                                                "The final identity and its private key never enter the server process.",
                                                "最终身份文档及其私钥不会进入服务器进程。",
                                            )}
                                        </p>
                                    </div>
                                    <dl class="boundary-list">
                                        <div>
                                            <dt>{text(language.get(), "Private key", "私钥")}</dt>
                                            <dd>{text(language.get(), "Browser only", "仅浏览器")}</dd>
                                        </div>
                                        <div>
                                            <dt>{text(language.get(), "Access token", "Access Token")}</dt>
                                            <dd>{text(language.get(), "Transient server memory", "服务器瞬时内存")}</dd>
                                        </div>
                                        <div>
                                            <dt>{text(language.get(), "Public key", "公钥")}</dt>
                                            <dd>{text(language.get(), "Sent to OpenAI", "发送至 OpenAI")}</dd>
                                        </div>
                                        <div>
                                            <dt>{text(language.get(), "Identity", "身份文档")}</dt>
                                            <dd>{text(language.get(), "Assembled in browser", "在浏览器中组装")}</dd>
                                        </div>
                                    </dl>
                                    <div class="boundary-note" role="status">
                                        <span class="boundary-note__mark" aria-hidden="true"></span>
                                        <span>{phase_status(language.get(), phase.get())}</span>
                                    </div>
                                </div>
                            }
                                .into_any()
                        }
                    }}
                </div>
            </section>

            <section class="explanation" aria-labelledby="how-title">
                <div class="explanation__copy">
                    <h2 id="how-title">{move || text(language.get(), "How it works", "工作方式")}</h2>
                    <p>
                        {move || {
                            text(
                                language.get(),
                                "One browser session carries the operation from key generation to a downloadable credential document.",
                                "一个浏览器会话完成从密钥生成到可下载凭据文档的全部操作。",
                            )
                        }}
                    </p>
                </div>
                <ol class="method-list">
                    <li>
                        <span>"01"</span>
                        <div>
                            <h3>{move || text(language.get(), "Generate", "生成")}</h3>
                            <p>{move || text(language.get(), "Create Ed25519 key material in WASM.", "在 WASM 中创建 Ed25519 密钥材料。")}</p>
                        </div>
                    </li>
                    <li>
                        <span>"02"</span>
                        <div>
                            <h3>{move || text(language.get(), "Register", "注册")}</h3>
                            <p>{move || text(language.get(), "Send the token and public key to the same-origin API.", "将 Token 和公钥发送到同源 API。")}</p>
                        </div>
                    </li>
                    <li>
                        <span>"03"</span>
                        <div>
                            <h3>{move || text(language.get(), "Assemble", "组装")}</h3>
                            <p>{move || text(language.get(), "Combine the runtime response with the private key locally.", "在本地将 Runtime 响应与私钥组合。")}</p>
                        </div>
                    </li>
                </ol>
            </section>

            <section class="security" aria-labelledby="security-title">
                <div class="security__statement">
                    <h2 id="security-title">{move || text(language.get(), "Security by data placement", "通过数据位置保障安全")}</h2>
                </div>
                <div class="security__copy">
                    <p>
                        {move || {
                            text(
                                language.get(),
                                "No localStorage. No cookies. No private-key upload. The access token remains available after a failed request so you can correct or retry it, and is erased from the form after success.",
                                "不使用 localStorage，不使用 Cookie，不上传私钥。请求失败后 Access Token 会保留以便修正或重试；成功后会从表单中清除。",
                            )
                        }}
                    </p>
                </div>
            </section>
        </main>

        <footer class="footer">
            <span>"AGENT IDENTITY · WEB"</span>
            <a href=GITHUB_URL target="_blank" rel="noreferrer">"GitHub"</a>
        </footer>
    }
}

fn text(language: Language, english: &'static str, chinese: &'static str) -> &'static str {
    match language {
        Language::En => english,
        Language::Zh => chinese,
    }
}

fn is_processing(phase: Phase) -> bool {
    matches!(
        phase,
        Phase::GeneratingKey | Phase::PreparingRequest | Phase::Registering | Phase::Assembling
    )
}

#[cfg(any(feature = "hydrate", test))]
fn phase_step(phase: Phase) -> usize {
    match phase {
        Phase::Idle | Phase::GeneratingKey | Phase::Error => 0,
        Phase::PreparingRequest => 1,
        Phase::Registering => 2,
        Phase::Assembling | Phase::Ready => 3,
    }
}

fn step_state(phase: Phase, index: usize, failed_step: usize) -> &'static str {
    let active = match phase {
        Phase::Error => failed_step,
        Phase::Idle => 0,
        Phase::GeneratingKey => 0,
        Phase::PreparingRequest => 1,
        Phase::Registering => 2,
        Phase::Assembling => 3,
        Phase::Ready => 4,
    };

    if phase == Phase::Error && index == failed_step {
        "error"
    } else if index < active {
        "complete"
    } else if index == active && active < 4 {
        "current"
    } else {
        "pending"
    }
}

fn step_label(language: Language, index: usize) -> &'static str {
    match (language, index) {
        (Language::En, 0) => "Generate key material",
        (Language::En, 1) => "Prepare secure request",
        (Language::En, 2) => "Register with OpenAI",
        (Language::En, _) => "Assemble identity",
        (Language::Zh, 0) => "生成密钥材料",
        (Language::Zh, 1) => "准备安全请求",
        (Language::Zh, 2) => "向 OpenAI 注册",
        (Language::Zh, _) => "组装身份文档",
    }
}

fn phase_status(language: Language, phase: Phase) -> &'static str {
    match phase {
        Phase::Idle => text(
            language,
            "Ready for local key generation",
            "已准备好在本地生成密钥",
        ),
        Phase::GeneratingKey => text(
            language,
            "Generating Ed25519 key material",
            "正在生成 Ed25519 密钥材料",
        ),
        Phase::PreparingRequest => text(
            language,
            "Preparing the same-origin request",
            "正在准备同源请求",
        ),
        Phase::Registering => text(language, "Registering the public key", "正在注册公钥"),
        Phase::Assembling => text(
            language,
            "Assembling the identity in browser",
            "正在浏览器中组装身份文档",
        ),
        Phase::Ready => text(language, "Identity ready", "身份已就绪"),
        Phase::Error => text(
            language,
            "Registration stopped; review the message",
            "注册已停止，请查看错误信息",
        ),
    }
}

fn localized_error(language: Language, code: &str) -> &'static str {
    match (language, code) {
        (Language::En, "TOKEN_REQUIRED") => "Enter an access token before generating an identity.",
        (Language::Zh, "TOKEN_REQUIRED") => "请输入 Access Token 后再生成身份。",
        (Language::En, "INVALID_TOKEN") => {
            "The access token is invalid. Obtain a new token and retry."
        }
        (Language::Zh, "INVALID_TOKEN") => "Access Token 无效，请获取新 Token 后重试。",
        (Language::En, "TOKEN_EXPIRED") => {
            "The access token has expired. Obtain a fresh token and retry."
        }
        (Language::Zh, "TOKEN_EXPIRED") => "Access Token 已过期，请获取新 Token 后重试。",
        (Language::En, "REGISTRATION_REJECTED") => {
            "OpenAI rejected this registration. Verify the account and retry."
        }
        (Language::Zh, "REGISTRATION_REJECTED") => "OpenAI 拒绝了本次注册，请核对账号后重试。",
        (Language::En, "UPSTREAM_UNAVAILABLE") => {
            "The registration service is unavailable. Retry in a moment."
        }
        (Language::Zh, "UPSTREAM_UNAVAILABLE") => "注册服务暂时不可用，请稍后重试。",
        (Language::En, "INVALID_REQUEST") => {
            "The registration request was invalid. Generate a new key and retry."
        }
        (Language::Zh, "INVALID_REQUEST") => "注册请求无效，请重新生成密钥后重试。",
        (Language::En, "KEY_GENERATION_FAILED") => {
            "The browser could not generate key material. Reload and retry."
        }
        (Language::Zh, "KEY_GENERATION_FAILED") => "浏览器无法生成密钥材料，请刷新后重试。",
        (Language::En, "INVALID_RESPONSE") => {
            "The server returned an invalid response. Retry in a moment."
        }
        (Language::Zh, "INVALID_RESPONSE") => "服务器返回了无效响应，请稍后重试。",
        (Language::En, "NETWORK_ERROR") => {
            "The request could not reach the server. Check the connection and retry."
        }
        (Language::Zh, "NETWORK_ERROR") => "请求无法到达服务器，请检查网络后重试。",
        (Language::En, "INTERNAL_ERROR") => {
            "The server could not complete the request. Retry in a moment."
        }
        (Language::Zh, "INTERNAL_ERROR") => "服务器无法完成请求，请稍后重试。",
        (Language::En, "DOWNLOAD_FAILED") => "The download could not start. Copy the JSON instead.",
        (Language::Zh, "DOWNLOAD_FAILED") => "无法开始下载，请改用复制 JSON。",
        (Language::En, _) => "The identity could not be generated. Retry in a moment.",
        (Language::Zh, _) => "无法生成身份，请稍后重试。",
    }
}

#[cfg(feature = "hydrate")]
async fn generate_identity(
    access_token: String,
    set_phase: WriteSignal<Phase>,
) -> Result<IdentityOutput, String> {
    leptos::task::tick().await;
    let key_material =
        crate::generate_key_material().map_err(|_| "KEY_GENERATION_FAILED".to_owned())?;
    set_phase.set(Phase::PreparingRequest);
    leptos::task::tick().await;

    let request = RegisterRuntimeRequest {
        access_token,
        agent_public_key: key_material.public_key_ssh().to_owned(),
    };
    set_phase.set(Phase::Registering);

    let response = register_runtime(&request).await?;
    drop(request);
    set_phase.set(Phase::Assembling);
    leptos::task::tick().await;

    let claims = crate::AccountClaims {
        account_id: response.account.account_id,
        chatgpt_user_id: response.account.chatgpt_user_id,
        email: response.account.email,
        plan_type: response.account.plan_type,
        is_fedramp: false,
    };
    let document =
        crate::build_identity_document(&response.agent_runtime_id, &key_material, &claims);
    let mut json =
        serde_json::to_string_pretty(&document).map_err(|_| "INVALID_RESPONSE".to_owned())?;
    json.push('\n');

    Ok(IdentityOutput {
        filename: crate::output_filename(&claims.email, &claims.plan_type),
        json,
    })
}

#[cfg(feature = "hydrate")]
async fn register_runtime(
    request: &RegisterRuntimeRequest,
) -> Result<RegisterRuntimeResponse, String> {
    let request_json = serde_json::to_string(request).map_err(|_| "INVALID_REQUEST".to_owned())?;
    let response = browser_api::post_registration(request_json)
        .await
        .map_err(|_| "NETWORK_ERROR".to_owned())?;

    if (200..300).contains(&response.status) {
        serde_json::from_str(&response.body).map_err(|_| "INVALID_RESPONSE".to_owned())
    } else {
        let envelope: ApiErrorEnvelope =
            serde_json::from_str(&response.body).map_err(|_| "INVALID_RESPONSE".to_owned())?;
        Err(envelope.error.code.as_str().to_owned())
    }
}

#[cfg(feature = "hydrate")]
fn start_copy(
    identity_output: IdentityOutput,
    copy_state: ReadSignal<CopyState>,
    set_copy_state: WriteSignal<CopyState>,
) {
    wasm_bindgen_futures::spawn_local(async move {
        match copy_text(&identity_output.json).await {
            Ok(()) => {
                set_copy_state.set(CopyState::Copied);
                leptos::leptos_dom::helpers::set_timeout(
                    move || {
                        if copy_state.get_untracked() == CopyState::Copied {
                            set_copy_state.set(CopyState::Idle);
                        }
                    },
                    std::time::Duration::from_millis(2_500),
                );
            }
            Err(()) => set_copy_state.set(CopyState::Error),
        }
    });
}

#[cfg(not(feature = "hydrate"))]
fn start_copy(
    _identity_output: IdentityOutput,
    _copy_state: ReadSignal<CopyState>,
    _set_copy_state: WriteSignal<CopyState>,
) {
}

#[cfg(feature = "hydrate")]
async fn copy_text(value: &str) -> Result<(), ()> {
    let window = web_sys::window().ok_or(())?;
    wasm_bindgen_futures::JsFuture::from(window.navigator().clipboard().write_text(value))
        .await
        .map(|_| ())
        .map_err(|_| ())
}

#[cfg(feature = "hydrate")]
fn start_download(identity_output: &IdentityOutput) -> Result<(), ()> {
    browser_api::download_json(&identity_output.json, &identity_output.filename).map_err(|_| ())
}

#[cfg(not(feature = "hydrate"))]
fn start_download(_identity_output: &IdentityOutput) -> Result<(), ()> {
    Ok(())
}

#[cfg(feature = "hydrate")]
fn browser_language() -> Language {
    web_sys::window()
        .and_then(|window| window.navigator().language())
        .filter(|language| language.to_ascii_lowercase().starts_with("zh"))
        .map_or(Language::En, |_| Language::Zh)
}

#[cfg(not(feature = "hydrate"))]
fn browser_language() -> Language {
    Language::En
}

#[cfg(feature = "hydrate")]
fn set_document_language(language: Language) {
    let language = match language {
        Language::En => "en",
        Language::Zh => "zh-CN",
    };
    browser_api::set_document_language(language);
}

#[cfg(not(feature = "hydrate"))]
fn set_document_language(_language: Language) {}

#[cfg(feature = "hydrate")]
mod browser_api {
    use js_sys::Array;
    use wasm_bindgen::{JsCast as _, JsValue};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{
        Blob, BlobPropertyBag, HtmlAnchorElement, RequestCache, RequestCredentials, RequestInit,
        Response, Url,
    };

    pub struct FetchResult {
        pub status: u16,
        pub body: String,
    }

    pub fn set_document_language(language: &str) {
        if let Some(root) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.document_element())
        {
            root.set_attribute("lang", language).ok();
        }
    }

    pub async fn post_registration(body: String) -> Result<FetchResult, JsValue> {
        let headers = web_sys::Headers::new()?;
        headers.set("Content-Type", "application/json")?;

        let init = RequestInit::new();
        init.set_method("POST");
        init.set_credentials(RequestCredentials::Omit);
        init.set_cache(RequestCache::NoStore);
        init.set_headers_headers(&headers);
        init.set_body(&JsValue::from_str(&body));

        let window = web_sys::window().ok_or_else(|| JsValue::from_str("window unavailable"))?;
        let response = JsFuture::from(window.fetch_with_str_and_init("/api/agent-runtimes", &init))
            .await?
            .dyn_into::<Response>()?;
        let status = response.status();
        let body = JsFuture::from(response.text()?)
            .await?
            .as_string()
            .ok_or_else(|| JsValue::from_str("response body is not text"))?;

        Ok(FetchResult { status, body })
    }

    pub fn download_json(contents: &str, filename: &str) -> Result<(), JsValue> {
        let parts = Array::new();
        parts.push(&JsValue::from_str(contents));
        let options = BlobPropertyBag::new();
        options.set_type("application/json;charset=utf-8");
        let blob = Blob::new_with_str_sequence_and_options(&parts, &options)?;
        let url = Url::create_object_url_with_blob(&blob)?;

        let window = web_sys::window().ok_or_else(|| JsValue::from_str("window unavailable"))?;
        let document = window
            .document()
            .ok_or_else(|| JsValue::from_str("document unavailable"))?;
        let anchor = document
            .create_element("a")?
            .dyn_into::<HtmlAnchorElement>()?;
        anchor.set_href(&url);
        anchor.set_download(filename);
        anchor.set_hidden(true);
        document
            .body()
            .ok_or_else(|| JsValue::from_str("document body unavailable"))?
            .append_child(&anchor)?;
        anchor.click();
        anchor.remove();
        Url::revoke_object_url(&url)
    }
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    leptos::mount::hydrate_body(App);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_failure_marks_the_registration_step() {
        assert_eq!(phase_step(Phase::Registering), 2);
        assert_eq!(step_state(Phase::Error, 0, 2), "complete");
        assert_eq!(step_state(Phase::Error, 2, 2), "error");
        assert_eq!(step_state(Phase::Error, 3, 2), "pending");
    }
}
