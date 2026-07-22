# agentidentity

[English](README.md) | 简体中文

`agentidentity` 是一个小型 Rust 命令行程序。它接受一个由用户手动提供的 ChatGPT JWT Access Token，注册 OpenAI Agent Identity Runtime，并生成一份可复用的 Agent Identity JSON 文件。

## 程序会做什么

1. 通过终端可见输入读取一个 Access Token。
2. 检查 JWT 是否为三段式结构，并验证各段编码、JSON Claims 和过期时间。
3. 提取账号 ID、ChatGPT 用户 ID、邮箱、套餐类型和 FedRAMP 标记。
4. 在本地随机生成 Ed25519 密钥对。
5. 将公钥发送到固定的 OpenAI 注册接口。
6. 将接口返回的 Runtime ID、私钥和账号信息写入当前工作目录中的 JSON 文件。

程序不接受命令行 Token 参数，也不会把 Token 写入输出文件。JWT Claims 会在本地解析，但不会在本地验证 JWT 签名；Token 的真实性由经过身份认证的 HTTPS 注册请求验证。

## 使用条件

- 已安装 Rust 工具链和 Cargo，建议使用当前稳定版 Rust。
- 拥有一个有效、未过期、三段式的 ChatGPT JWT Access Token，并且你有权使用对应账号。
- 网络能够访问：

  ```text
  https://auth.openai.com/api/accounts/v1/agent/register
  ```

本程序不会替你登录 ChatGPT，也不会替你获取 Access Token。登录 ChatGPT 后，可以在浏览器中打开 <https://chatgpt.com/api/auth/session>，然后从返回的 JSON 中复制 `accessToken` 字段的值。Token 属于敏感凭据，请只使用你有权访问的账号。

## 构建程序

进入项目目录后执行：

```sh
cargo build --release
```

构建后的可执行文件位于：

```text
target/release/agentidentity
```

Windows 下的文件为 `target\release\agentidentity.exe`。

## 使用方法

### 直接通过 Cargo 运行

```sh
cargo run --release
```

### 运行已构建的程序

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

## 通过环境变量设置网络代理

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

### Linux 和 macOS

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

### Windows PowerShell

为当前 PowerShell 会话设置代理：

```powershell
$env:HTTPS_PROXY = "http://127.0.0.1:7890"
.\target\release\agentidentity.exe
```

使用完毕后删除变量：

```powershell
Remove-Item Env:HTTPS_PROXY
```

### Windows 命令提示符

```bat
set HTTPS_PROXY=http://127.0.0.1:7890
target\release\agentidentity.exe
```

使用完毕后清除变量：

```bat
set HTTPS_PROXY=
```

### 需要用户名和密码的 HTTP 代理

可以把代理用户名和密码写入代理 URL：

```sh
export HTTPS_PROXY=http://username:password@proxy.example.com:8080
cargo run --release
```

如果用户名或密码含有特殊字符，需要进行 URL 百分号编码。例如，`@` 编码为 `%40`，`:` 编码为 `%3A`。

请注意：环境变量中的代理凭据可能会被同一台机器上的其他工具或进程看到。如果条件允许，优先使用无需在 URL 中写入凭据的本地代理，或者使用更安全的凭据管理方式。

### 代理使用注意事项

- 访问 HTTPS 目标时使用 `http://127.0.0.1:7890` 形式的 HTTP 代理是正常的。客户端会要求代理建立 HTTP `CONNECT` 隧道，程序与 OpenAI 之间的连接仍由 TLS 加密。
- 确保 `auth.openai.com` 没有出现在 `NO_PROXY` 中，否则注册请求会绕过代理直连。
- 程序已禁用 HTTP 重定向，Access Token 只会发送到固定的注册 URL。
- 当前构建没有启用 Reqwest 可选的 SOCKS 功能。请使用 HTTP/HTTPS CONNECT 代理，不要填写 `socks5://` 代理 URL。
- 某些公司代理会拦截 TLS，可能导致证书验证失败。不要关闭 TLS 验证，请使用受系统信任且配置正确的代理。

## 输出文件

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

## 安全说明

- Token 输入内容可见。请在私密终端中运行，必要时在运行后清理终端，不要通过屏幕共享、录屏、截图、Issue 或命令行参数暴露 Token。
- 程序运行时使用可清零内存保存 Token，并且不会把 Token 写入生成的 JSON。
- 生成的 JSON 含有可复用私钥，应把整个文件视为敏感凭据。
- 按项目要求，程序不会主动修改输出文件权限。请根据操作系统和运行环境自行保护文件。
- 项目的 `.gitignore` 已忽略最终输出文件和临时输出文件，但 Git 忽略规则不能代替文件访问控制。
- HTTP 错误响应正文可能包含敏感信息，因此程序不会显示响应正文。

## 常见问题

### `token must be a three-part JWT`

输入内容为空、不完整、带有引号，或者不是 JWT Access Token。登录 ChatGPT 后打开 <https://chatgpt.com/api/auth/session>，复制完整的 `accessToken` 值；它应为 `header.payload.signature` 格式，输入时不要添加引号。

### `token has expired`

Token 已过期。请获取一个新的、经过授权的 Access Token 后重新运行程序。

### `token is missing ...`

JWT 中缺少程序所需的 ChatGPT 账号 Claim。请确认你提供的是正确类型的 Access Token。

### `failed to send Agent Identity registration request`

检查网络、DNS、防火墙、代理设置和系统时间。如果使用代理，请确认代理地址正确，并确保 `auth.openai.com` 没有被 `NO_PROXY` 排除。

### `Agent Identity registration failed with HTTP 401 Unauthorized`

Token 可能无效、已过期、已撤销，或者没有权限执行注册请求。出于安全考虑，程序不会打印服务器返回的响应正文。

### 使用代理时出现证书或 TLS 错误

代理可能使用了系统不信任的证书拦截 TLS。请改用配置正确且受信任的代理；程序不提供关闭证书验证的选项。

## 开发检查

```sh
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
```
