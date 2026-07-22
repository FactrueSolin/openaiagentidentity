# agentidentity

English | [简体中文](README.zh.md)

`agentidentity` provides a Web application and a command-line application for registering an OpenAI Agent Runtime from a manually supplied ChatGPT JWT access token and producing a reusable Agent Identity JSON document.

## Web application

### Architecture and data boundary

The Web application uses Leptos on Axum with server-side rendering (SSR) followed by browser hydration. Axum renders the initial page and serves the generated JavaScript, WebAssembly, and CSS assets; the hydrated Leptos frontend then handles the interactive identity-generation flow.

The browser generates the Ed25519 key pair and assembles the final identity document. Only the access token and public key are sent to the application server. The server handles them transiently for the registration request, forwards the token and public key to OpenAI over HTTPS, and returns the runtime and account metadata needed by the browser. The private key never leaves the browser and the server does not create the final identity document.

### Requirements

- A Rust toolchain with Cargo installed. The current stable Rust release is recommended.
- [`cargo-leptos`](https://github.com/leptos-rs/cargo-leptos):

  ```sh
  cargo install cargo-leptos --locked
  ```

- The WebAssembly compilation target:

  ```sh
  rustup target add wasm32-unknown-unknown
  ```

- A modern browser with WebAssembly, JavaScript modules, Fetch, Blob downloads, and the Clipboard API. Current versions of Chrome, Edge, Firefox, and Safari are recommended. Clipboard access may require user permission and a secure context such as `https://` or localhost.

### Development

From the repository directory, start the development server and asset watcher:

```sh
cargo leptos watch
```

Then open <http://127.0.0.1:3000>. Changes to the Rust frontend, server, styles, and public assets are rebuilt automatically.

### Release build and server

Build the SSR server, hydrated frontend, and site assets:

```sh
cargo leptos build --release
```

From the repository directory, run the release server with the generated `target/site` directory present:

```sh
./target/release/agentidentity-web
```

The server binary is `target/release/agentidentity-web` and the browser assets are under `target/site`. A deployment must include both. If the binary is copied elsewhere, run it from a directory with this layout:

```text
application/
├── agentidentity-web
└── target/
    └── site/
```

Copying and running the binary without `target/site` is not a complete deployment.

### GitHub Actions Ubuntu package

Every push runs the `Build Ubuntu package` workflow. It builds and tests the complete Web application on Ubuntu 22.04 for x86_64, produces a statically linked musl server binary, performs a packaged-server smoke test, and uploads an artifact named `agentidentity-web-linux-x86_64-<commit-sha>`.

Download the artifact from the commit's workflow run in the repository's **Actions** tab. Artifacts are retained for 14 days. GitHub downloads the artifact as an outer ZIP file; extract it first:

```sh
unzip agentidentity-web-linux-x86_64-COMMIT_SHA.zip
```

The ZIP contains:

```text
agentidentity-web-linux-x86_64.tar.gz
agentidentity-web-linux-x86_64.tar.gz.sha256
```

Verify and extract the deployment archive on an Ubuntu x86_64 server:

```sh
sha256sum --check agentidentity-web-linux-x86_64.tar.gz.sha256
tar -xzf agentidentity-web-linux-x86_64.tar.gz
cd agentidentity-web-linux-x86_64
cp .env.example .env
```

Edit `.env`, then start the server from that directory:

```sh
./agentidentity-web
```

The tarball includes all runtime application files: a statically linked x86_64 musl executable, `.env.example`, hydrated WebAssembly/JavaScript, styles, and fonts in the required `target/site` layout. The target Ubuntu server does not need Rust, `cargo-leptos`, glibc, or OpenSSL runtime libraries. Other CPU architectures are not produced by this workflow.

### Configuration

The Web server reads process environment variables first, then a `.env` file in its current working directory, then built-in defaults: environment > `.env` > default.

| Setting | Default | Description |
| --- | --- | --- |
| `HOST` | `127.0.0.1` | Listener IP address or hostname, without a port. |
| `PORT` | `3000` | Listener TCP port. |
| `PROXY_URL` | blank | Optional single `http://` or `https://` proxy used for all server outbound requests. |

Example `.env`:

```dotenv
HOST=127.0.0.1
PORT=3000
PROXY_URL=http://127.0.0.1:7890
```

A blank `PROXY_URL` explicitly selects a direct connection. In particular, a blank process environment value takes precedence over a proxy in `.env`. The Web server deliberately ignores `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, and `NO_PROXY`; only `PROXY_URL` controls its outbound proxy. This differs from the CLI proxy behavior documented below.

At startup, the server logs the effective `HOST`, `PORT`, and `PROXY_URL` and the source of each value. Proxy credentials and URL details are redacted in the log; a direct connection is logged as `direct`.

### HTTP API

The hydrated frontend calls:

```text
POST /api/agent-runtimes
```

This is a same-origin, frontend-oriented registration endpoint. It transiently accepts the access token and browser-generated public key and returns the registration result needed by the frontend. It is not a complete or general-purpose identity API: it does not generate or retain the private key, assemble the final identity document, or provide identity-management operations.

### Copy and download

After successful registration, the browser displays the final JSON document:

- **Copy** writes the complete JSON to the system clipboard. The browser may ask for clipboard permission; use **Download** if clipboard access is unavailable.
- **Download** creates the JSON file entirely in the browser and saves it using the generated, sanitized identity filename. The server does not receive the private key or downloaded document.

Both outputs contain a reusable private key. Treat copied clipboard contents and downloaded files as secrets.

### Public production deployment

The Axum server listens over plain HTTP. Any publicly reachable production deployment must put it behind an HTTPS reverse proxy and serve the page, assets, and `/api/agent-runtimes` from the same origin. Configure the reverse proxy, observability stack, and upstream services not to log request bodies: the API request body contains the access token and public key. Keep `target/site` beside the deployed server as described above.

## CLI application

`agentidentity` is also a small Rust command-line program that registers one OpenAI Agent Runtime from a manually supplied ChatGPT JWT access token. It extracts the required account metadata, generates a local Ed25519 key pair, registers the public key with OpenAI, and writes a reusable Agent Identity JSON file.

### What it does

1. Reads one access token from a visible terminal prompt.
2. Validates the three-part JWT structure, encoding, JSON claims, and expiration time.
3. Extracts the account ID, ChatGPT user ID, email, plan type, and FedRAMP flag.
4. Generates a random Ed25519 key pair locally.
5. Sends the public key to the fixed OpenAI registration endpoint.
6. Writes the returned runtime ID, private key, and account metadata to a JSON file in the current working directory.

The access token is never accepted as a command-line argument and is not written to the output file. JWT claims are decoded locally without signature verification; token authenticity is enforced by the authenticated HTTPS registration request.

### Requirements

- A Rust toolchain with Cargo installed. The current stable Rust release is recommended.
- A valid, unexpired, three-part ChatGPT JWT access token for an account you are authorized to use.
- Network access to:

  ```text
  https://auth.openai.com/api/accounts/v1/agent/register
  ```

This program does not perform a ChatGPT login or obtain an access token for you. While signed in to ChatGPT, open <https://chatgpt.com/api/auth/session> in your browser and copy the value of the `accessToken` field from the returned JSON. The token is sensitive and should only be used for an account you are authorized to access.

### Build

From the repository directory:

```sh
cargo build --release
```

The compiled executable will be located at:

```text
target/release/agentidentity
```

On Windows, the executable is `target\release\agentidentity.exe`.

### Usage

#### Run directly with Cargo

```sh
cargo run --release
```

#### Run the compiled executable

Linux or macOS:

```sh
./target/release/agentidentity
```

Windows PowerShell:

```powershell
.\target\release\agentidentity.exe
```

At startup, the program prints the network proxy selected for the OpenAI HTTPS request and then displays the token instructions and prompt:

```text
Network proxy: HTTPS_PROXY=http://127.0.0.1:7890
Get your access token from https://chatgpt.com/api/auth/session while signed in to ChatGPT.
Paste access token (input is visible):
```

Open <https://chatgpt.com/api/auth/session> while signed in to ChatGPT, copy the `accessToken` value from the returned JSON, paste the complete JWT access token, and press Enter. Input is intentionally visible so you can verify that the entire token was pasted. Do not add quotes around it, and avoid running the program where the terminal can be observed or recorded.

A successful run looks similar to:

```text
Network proxy: direct (no valid HTTPS proxy environment variable)
Get your access token from https://chatgpt.com/api/auth/session while signed in to ChatGPT.
Paste access token (input is visible): eyJ...
Registering Agent Identity with OpenAI...
Created agent-identity-person_example.com-team.json
```

The output file is created in the directory from which the executable was started, not necessarily beside the executable.

### Using a network proxy

The HTTP client reads standard proxy environment variables automatically. No command-line proxy option is required. At startup, the program reports the proxy selected for the fixed `auth.openai.com` HTTPS request, for example:

```text
Network proxy: HTTPS_PROXY=http://127.0.0.1:7890
```

If no applicable proxy is configured, or `NO_PROXY` bypasses `auth.openai.com`, it reports a direct connection. User information in authenticated proxy URLs is replaced with `***` in this status line so the proxy username and password are not printed.

Because the registration endpoint uses HTTPS, `HTTPS_PROXY` is the most relevant variable:

| Variable | Purpose |
| --- | --- |
| `HTTPS_PROXY` or `https_proxy` | Proxy for HTTPS destinations. Recommended for this program. |
| `HTTP_PROXY` or `http_proxy` | Proxy for plain HTTP destinations. The registration endpoint itself is HTTPS. |
| `ALL_PROXY` or `all_proxy` | Fallback proxy for both HTTP and HTTPS when a protocol-specific variable is not set. |
| `NO_PROXY` or `no_proxy` | Comma-separated destinations that must bypass the proxy. |

A protocol-specific proxy takes precedence over `ALL_PROXY`. If both uppercase and lowercase forms are present, avoid ambiguity by setting only one form.

#### Linux and macOS

Use a proxy for one invocation only:

```sh
HTTPS_PROXY=http://127.0.0.1:7890 cargo run --release
```

Or export it for the current shell session:

```sh
export HTTPS_PROXY=http://127.0.0.1:7890
cargo run --release
```

Remove it afterward if it is no longer needed:

```sh
unset HTTPS_PROXY
```

You can use `ALL_PROXY` as a fallback instead:

```sh
ALL_PROXY=http://127.0.0.1:7890 ./target/release/agentidentity
```

#### Windows PowerShell

Set the proxy for the current PowerShell session:

```powershell
$env:HTTPS_PROXY = "http://127.0.0.1:7890"
.\target\release\agentidentity.exe
```

Remove it afterward:

```powershell
Remove-Item Env:HTTPS_PROXY
```

#### Windows Command Prompt

```bat
set HTTPS_PROXY=http://127.0.0.1:7890
target\release\agentidentity.exe
```

Clear it afterward:

```bat
set HTTPS_PROXY=
```

#### Authenticated HTTP proxy

Credentials can be included in the proxy URL:

```sh
export HTTPS_PROXY=http://username:password@proxy.example.com:8080
cargo run --release
```

Percent-encode special characters in the username or password. For example, `@` becomes `%40` and `:` becomes `%3A`.

Be aware that proxy credentials stored in environment variables may be visible to other tools or processes on the same machine. Prefer a local proxy without embedded credentials or a secure credential mechanism when available.

#### Important proxy notes

- An HTTP proxy URL such as `http://127.0.0.1:7890` is normal for an HTTPS destination: the client asks the proxy to establish an HTTP `CONNECT` tunnel, while TLS still protects the connection to OpenAI.
- Make sure `auth.openai.com` is not listed in `NO_PROXY`; otherwise the request bypasses the proxy.
- Redirect following is disabled. The access token is sent only to the fixed registration URL.
- This build does not enable Reqwest's optional SOCKS feature. Use an HTTP/HTTPS CONNECT proxy rather than a `socks5://` proxy URL.
- TLS-intercepting corporate proxies may cause certificate validation errors. Do not disable TLS verification; use a trusted, correctly configured proxy.

### Output file

The generated filename is:

```text
agent-identity-{sanitized email}-{sanitized plan_type}.json
```

Every character outside `[A-Za-z0-9._-]` is replaced with `_`. For example:

```text
person+work@example.com + team / annual
```

becomes:

```text
agent-identity-person_work_example.com-team___annual.json
```

If a file with the same name already exists, it is atomically replaced through a temporary file in the same directory.

The output format is:

```json
{
  "auth_mode": "agentIdentity",
  "agent_identity": {
    "agent_runtime_id": "runtime ID returned by OpenAI",
    "agent_private_key": "Ed25519 PKCS#8 DER Base64 private key",
    "account_id": "ChatGPT account ID",
    "chatgpt_user_id": "ChatGPT user ID",
    "email": "account email",
    "plan_type": "account plan type"
  }
}
```

Version 0.1 does not create or include a `task_id`.

### Security notes

- Token input is visible. Run the program in a private terminal, clear the terminal afterward if needed, and do not expose the token through screen sharing, recording, screenshots, issue reports, or command-line arguments.
- The token is kept in zeroizing memory while the program runs and is not saved in the generated JSON.
- The generated JSON contains a reusable private key. Treat the entire file as a secret.
- The program intentionally does not change file permissions. Protect the output according to your operating system and environment.
- Output files and temporary output files are ignored by this repository's `.gitignore`, but do not rely on Git ignore rules as access control.
- HTTP error response bodies are not displayed because they may contain sensitive information.

### Troubleshooting

#### `token must be a three-part JWT`

The entered value is empty, truncated, quoted, or is not a JWT access token. While signed in to ChatGPT, open <https://chatgpt.com/api/auth/session>, copy the complete `accessToken` value in `header.payload.signature` form, and enter it without quotes.

#### `token has expired`

Obtain a new authorized access token and run the program again.

#### `token is missing ...`

The JWT does not contain one of the required ChatGPT account claims. Verify that you supplied the correct access token type.

#### `failed to send Agent Identity registration request`

Check network connectivity, DNS, firewall rules, proxy settings, and system time. If using a proxy, verify its address and ensure `auth.openai.com` is not excluded by `NO_PROXY`.

#### `Agent Identity registration failed with HTTP 401 Unauthorized`

The token may be invalid, expired, revoked, or not authorized for the registration request. The program deliberately does not print the server response body.

#### Certificate or TLS error through a proxy

The proxy may be intercepting TLS with an untrusted certificate. Use a correctly configured trusted proxy; the program does not provide an option to disable certificate verification.

### Development checks

```sh
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release
cargo leptos build --release
```
