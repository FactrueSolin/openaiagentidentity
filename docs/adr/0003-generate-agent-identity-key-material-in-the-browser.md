# Generate Agent Identity key material in the browser

The hydrated Leptos frontend will generate Ed25519 Key Material and assemble the final Agent Identity in the browser, sending only the public key and Access Token to Axum for OpenAI registration. This keeps the private key and complete Agent Identity out of the server process, at the cost of requiring browser cryptography in the WASM application and changing the HTTP API to return registration data rather than a finished identity document.
