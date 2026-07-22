# agentidentity

A small Rust CLI that registers an OpenAI Agent Identity Runtime from one manually entered ChatGPT access token.

## Build and run

```sh
cargo run --release
```

Paste the JWT access token at the hidden terminal prompt. The token is used only to read account claims and authenticate the registration request; it is not written to the output file.

The CLI writes a file in the current directory named:

```text
agent-identity-{email}-{plan_type}.json
```

Characters outside `[A-Za-z0-9._-]` are replaced with `_`. An existing file with the same name is replaced atomically.

## Output

```json
{
  "auth_mode": "agentIdentity",
  "agent_identity": {
    "agent_runtime_id": "...",
    "agent_private_key": "...",
    "account_id": "...",
    "chatgpt_user_id": "...",
    "email": "...",
    "plan_type": "..."
  }
}
```

`agent_private_key` is an Ed25519 PKCS#8 DER private key encoded with standard Base64. The generated JSON contains reusable private key material. Store and share it accordingly.

The registration request is sent only to:

```text
https://auth.openai.com/api/accounts/v1/agent/register
```
