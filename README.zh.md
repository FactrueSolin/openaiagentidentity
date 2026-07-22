# agentidentity

[English](README.md) | 简体中文

`agentidentity` 提供 Web 应用和命令行应用。两者都接受由用户手动提供的 ChatGPT JWT Access Token，用于注册 OpenAI Agent Runtime，并生成一份可复用的 Agent Identity JSON 文档。

## Web 应用

### 架构与数据边界

Web 应用在 Axum 上使用 Leptos，先进行服务端渲染（SSR），再在浏览器中 Hydration。Axum 渲染初始页面并提供生成的 JavaScript、WebAssembly 和 CSS 资源；Hydration 后的 Leptos 前端负责交互式身份生成流程。

浏览器生成 Ed25519 密钥对，并组装最终身份文档。只有 Access Token 和公钥会发送到应用服务器。服务器只在注册请求期间临时处理这两项数据，通过 HTTPS 将 Token 和公钥转发给 OpenAI，再把浏览器所需的 Runtime 和账号元数据返回给前端。私钥永远不会离开浏览器，服务器也不会创建最终身份文档。

### 使用条件

- 已安装 Rust 工具链和 Cargo，建议使用当前稳定版 Rust。
- [`cargo-leptos`](https://github.com/leptos-rs/cargo-leptos)：

  ```sh
  cargo install cargo-leptos --locked
  ```

- WebAssembly 编译目标：

  ```sh
  rustup target add wasm32-unknown-unknown
  ```

- 支持 WebAssembly、JavaScript Modules、Fetch、Blob 下载和 Clipboard API 的现代浏览器。建议使用当前版本的 Chrome、Edge、Firefox 或 Safari。访问剪贴板可能需要用户授权，并且要求使用 `https://` 或 localhost 等安全上下文。

### 开发

在项目目录中启动开发服务器和资源监听器：

```sh
cargo leptos watch
```

然后打开 <http://127.0.0.1:3000>。修改 Rust 前端、服务器、样式或公共资源后会自动重新构建。

### Release 构建与服务器

构建 SSR 服务器、Hydration 前端和站点资源：

```sh
cargo leptos build --release
```

在项目目录中运行 Release 服务器，并确保生成的 `target/site` 目录仍然存在：

```sh
./target/release/agentidentity-web
```

服务器二进制文件为 `target/release/agentidentity-web`，浏览器资源位于 `target/site`。部署时必须同时包含两者。如果把二进制文件复制到其他位置，应从具有以下布局的目录中启动：

```text
application/
├── agentidentity-web
└── target/
    └── site/
```

只复制并运行二进制文件而不提供 `target/site`，不能构成完整部署。

### 配置

Web 服务器依次读取当前进程的环境变量、当前工作目录中的 `.env` 文件和内置默认值，优先级为：环境变量 > `.env` > 默认值。

| 配置项 | 默认值 | 说明 |
| --- | --- | --- |
| `HOST` | `127.0.0.1` | 监听 IP 地址或主机名，不包含端口。 |
| `PORT` | `3000` | 监听的 TCP 端口。 |
| `PROXY_URL` | 空 | 可选的单个 `http://` 或 `https://` 代理，用于服务器的所有出站请求。 |

`.env` 示例：

```dotenv
HOST=127.0.0.1
PORT=3000
PROXY_URL=http://127.0.0.1:7890
```

空白的 `PROXY_URL` 会明确选择直连。特别是，当进程环境变量中的值为空白时，它仍然优先于 `.env` 中的代理设置。Web 服务器会主动忽略 `HTTP_PROXY`、`HTTPS_PROXY`、`ALL_PROXY` 和 `NO_PROXY`；只有 `PROXY_URL` 能控制其出站代理。这一点与下文所述的 CLI 代理行为不同。

服务器启动时会记录最终生效的 `HOST`、`PORT` 和 `PROXY_URL`，以及每项配置的来源。日志中的代理凭据和 URL 细节会被隐藏；使用直连时会记录为 `direct`。

### HTTP API

Hydration 后的前端会调用：

```text
POST /api/agent-runtimes
```

这是一个面向前端、要求同源调用的注册接口。它临时接收 Access Token 和浏览器生成的公钥，并返回前端所需的注册结果。它不是完整或通用的身份 API：它不会生成或保存私钥，不会组装最终身份文档，也不提供身份管理操作。

### 复制与下载

注册成功后，浏览器会显示最终 JSON 文档：

- **复制**会把完整 JSON 写入系统剪贴板。浏览器可能要求剪贴板权限；如果无法访问剪贴板，请使用**下载**。
- **下载**会完全在浏览器中创建 JSON 文件，并使用程序生成且经过清理的身份文件名进行保存。服务器不会收到私钥或下载的文档。

两种输出都包含可复用的私钥。请把剪贴板中的副本和下载文件都视为敏感凭据。

### 公网生产部署

Axum 服务器监听的是普通 HTTP。任何可从公网访问的生产部署都必须把它置于 HTTPS 反向代理之后，并从同一 Origin 提供页面、资源和 `/api/agent-runtimes`。必须配置反向代理、可观测性系统和上游服务，不要记录请求正文：API 请求正文中含有 Access Token 和公钥。还应按照上文说明，在部署服务器时一并提供 `target/site`。

## CLI 应用

`agentidentity` 同时也是一个小型 Rust 命令行程序。它接受一个由用户手动提供的 ChatGPT JWT Access Token，注册 OpenAI Agent Runtime，并生成一份可复用的 Agent Identity JSON 文件。

### 程序会做什么

1. 通过终端可见输入读取一个 Access Token。
2. 检查 JWT 是否为三段式结构，并验证各段编码、JSON Claims 和过期时间。
3. 提取账号 ID、ChatGPT 用户 ID、邮箱、套餐类型和 FedRAMP 标记。
4. 在本地随机生成 Ed25519 密钥对。
5. 将公钥发送到固定的 OpenAI 注册接口。
6. 将接口返回的 Runtime ID、私钥和账号信息写入当前工作目录中的 JSON 文件。

程序不接受命令行 Token 参数，也不会把 Token 写入输出文件。JWT Claims 会在本地解析，但不会在本地验证 JWT 签名；Token 的真实性由经过身份认证的 HTTPS 注册请求验证。

### 使用条件

- 已安装 Rust 工具链和 Cargo，建议使用当前稳定版 Rust。
- 拥有一个有效、未过期、三段式的 ChatGPT JWT Access Token，并且你有权使用对应账号。
- 网络能够访问：

  ```text
  https://auth.openai.com/api/accounts/v1/agent/register
  ```

本程序不会替你登录 ChatGPT，也不会替你获取 Access Token。登录 ChatGPT 后，可以在浏览器中打开 <https://chatgpt.com/api/auth/session>，然后从返回的 JSON 中复制 `accessToken` 字段的值。Token 属于敏感凭据，请只使用你有权访问的账号。

### 构建程序

进入项目目录后执行：

```sh
cargo build --release
```

构建后的可执行文件位于：

```text
target/release/agentidentity
```

Windows 下的文件为 `target\release\agentidentity.exe`。

### 使用方法

#### 直接通过 Cargo 运行

```sh
cargo run --release
```

#### 运行已构建的程序

Linux 或 macOS：

```sh
./target/release/agentidentity
```

Windows PowerShell：

```powershell
.\target\release\agentidentity.exe
```

程序启动后会先输出 OpenAI HTTPS 请求将使用的网络代理，然后显示 Token 获取说明和输入提示：

```text
Network proxy: HTTPS_PROXY=http://127.0.0.1:7890
Get your access token from https://chatgpt.com/api/auth/session while signed in to ChatGPT.
Paste access token (input is visible):
```

先登录 ChatGPT，再打开 <https://chatgpt.com/api/auth/session>，从返回的 JSON 中复制 `accessToken` 值。粘贴完整的 JWT Access Token 后按 Enter。输入内容会直接显示，以便确认 Token 是否完整粘贴。请不要在 Token 两侧添加引号，也不要在终端可能被他人看到、共享或录制时运行程序。

成功运行时会看到类似输出：

```text
Network proxy: direct (no valid HTTPS proxy environment variable)
Get your access token from https://chatgpt.com/api/auth/session while signed in to ChatGPT.
Paste access token (input is visible): eyJ...
Registering Agent Identity with OpenAI...
Created agent-identity-person_example.com-team.json
```

输出文件会生成在启动程序时所在的当前目录中，不一定与可执行文件位于同一目录。

### 通过环境变量设置网络代理

程序使用的 HTTP 客户端会自动读取标准代理环境变量，不需要提供额外的代理命令行参数。程序启动时会输出固定 `auth.openai.com` HTTPS 请求实际选中的代理，例如：

```text
Network proxy: HTTPS_PROXY=http://127.0.0.1:7890
```

如果没有适用的代理，或者 `NO_PROXY` 排除了 `auth.openai.com`，程序会提示使用直连。对于带认证信息的代理 URL，状态提示会把用户信息替换为 `***`，不会输出代理用户名和密码。

注册接口使用 HTTPS，因此最常用的是 `HTTPS_PROXY`：

| 环境变量 | 作用 |
| --- | --- |
| `HTTPS_PROXY` 或 `https_proxy` | 为 HTTPS 目标设置代理，本程序推荐使用。 |
| `HTTP_PROXY` 或 `http_proxy` | 为普通 HTTP 目标设置代理；注册接口本身是 HTTPS。 |
| `ALL_PROXY` 或 `all_proxy` | 当没有设置协议专用代理时，作为 HTTP 和 HTTPS 的通用后备代理。 |
| `NO_PROXY` 或 `no_proxy` | 使用逗号分隔不经过代理的目标地址。 |

协议专用变量的优先级高于 `ALL_PROXY`。为避免歧义，不建议同时设置同一个变量的大写和小写形式。

#### Linux 和 macOS

只让单次命令使用代理：

```sh
HTTPS_PROXY=http://127.0.0.1:7890 cargo run --release
```

也可以为当前 Shell 会话设置代理：

```sh
export HTTPS_PROXY=http://127.0.0.1:7890
cargo run --release
```

不再需要时删除变量：

```sh
unset HTTPS_PROXY
```

也可以使用 `ALL_PROXY` 作为后备代理：

```sh
ALL_PROXY=http://127.0.0.1:7890 ./target/release/agentidentity
```

#### Windows PowerShell

为当前 PowerShell 会话设置代理：

```powershell
$env:HTTPS_PROXY = "http://127.0.0.1:7890"
.\target\release\agentidentity.exe
```

使用完毕后删除变量：

```powershell
Remove-Item Env:HTTPS_PROXY
```

#### Windows 命令提示符

```bat
set HTTPS_PROXY=http://127.0.0.1:7890
target\release\agentidentity.exe
```

使用完毕后清除变量：

```bat
set HTTPS_PROXY=
```

#### 需要用户名和密码的 HTTP 代理

可以把代理用户名和密码写入代理 URL：

```sh
export HTTPS_PROXY=http://username:password@proxy.example.com:8080
cargo run --release
```

如果用户名或密码含有特殊字符，需要进行 URL 百分号编码。例如，`@` 编码为 `%40`，`:` 编码为 `%3A`。

请注意：环境变量中的代理凭据可能会被同一台机器上的其他工具或进程看到。如果条件允许，优先使用无需在 URL 中写入凭据的本地代理，或者使用更安全的凭据管理方式。

#### 代理使用注意事项

- 访问 HTTPS 目标时使用 `http://127.0.0.1:7890` 形式的 HTTP 代理是正常的。客户端会要求代理建立 HTTP `CONNECT` 隧道，程序与 OpenAI 之间的连接仍由 TLS 加密。
- 确保 `auth.openai.com` 没有出现在 `NO_PROXY` 中，否则注册请求会绕过代理直连。
- 程序已禁用 HTTP 重定向，Access Token 只会发送到固定的注册 URL。
- 当前构建没有启用 Reqwest 可选的 SOCKS 功能。请使用 HTTP/HTTPS CONNECT 代理，不要填写 `socks5://` 代理 URL。
- 某些公司代理会拦截 TLS，可能导致证书验证失败。不要关闭 TLS 验证，请使用受系统信任且配置正确的代理。

### 输出文件

生成的文件名格式为：

```text
agent-identity-{处理后的 email}-{处理后的 plan_type}.json
```

文件名中不属于 `[A-Za-z0-9._-]` 的字符都会被替换为 `_`。例如：

```text
person+work@example.com + team / annual
```

会生成：

```text
agent-identity-person_work_example.com-team___annual.json
```

如果同名文件已经存在，程序会先在同一目录写入临时文件，再通过原子重命名直接替换旧文件。

输出 JSON 格式如下：

```json
{
  "auth_mode": "agentIdentity",
  "agent_identity": {
    "agent_runtime_id": "OpenAI 返回的 Runtime ID",
    "agent_private_key": "Ed25519 PKCS#8 DER Base64 私钥",
    "account_id": "ChatGPT 账号 ID",
    "chatgpt_user_id": "ChatGPT 用户 ID",
    "email": "账号邮箱",
    "plan_type": "账号套餐类型"
  }
}
```

0.1 版本不会创建或输出 `task_id`。

### 安全说明

- Token 输入内容可见。请在私密终端中运行，必要时在运行后清理终端，不要通过屏幕共享、录屏、截图、Issue 或命令行参数暴露 Token。
- 程序运行时使用可清零内存保存 Token，并且不会把 Token 写入生成的 JSON。
- 生成的 JSON 含有可复用私钥，应把整个文件视为敏感凭据。
- 按项目要求，程序不会主动修改输出文件权限。请根据操作系统和运行环境自行保护文件。
- 项目的 `.gitignore` 已忽略最终输出文件和临时输出文件，但 Git 忽略规则不能代替文件访问控制。
- HTTP 错误响应正文可能包含敏感信息，因此程序不会显示响应正文。

### 常见问题

#### `token must be a three-part JWT`

输入内容为空、不完整、带有引号，或者不是 JWT Access Token。登录 ChatGPT 后打开 <https://chatgpt.com/api/auth/session>，复制完整的 `accessToken` 值；它应为 `header.payload.signature` 格式，输入时不要添加引号。

#### `token has expired`

Token 已过期。请获取一个新的、经过授权的 Access Token 后重新运行程序。

#### `token is missing ...`

JWT 中缺少程序所需的 ChatGPT 账号 Claim。请确认你提供的是正确类型的 Access Token。

#### `failed to send Agent Identity registration request`

检查网络、DNS、防火墙、代理设置和系统时间。如果使用代理，请确认代理地址正确，并确保 `auth.openai.com` 没有被 `NO_PROXY` 排除。

#### `Agent Identity registration failed with HTTP 401 Unauthorized`

Token 可能无效、已过期、已撤销，或者没有权限执行注册请求。出于安全考虑，程序不会打印服务器返回的响应正文。

#### 使用代理时出现证书或 TLS 错误

代理可能使用了系统不信任的证书拦截 TLS。请改用配置正确且受信任的代理；程序不提供关闭证书验证的选项。

### 开发检查

```sh
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release
cargo leptos build --release
```
