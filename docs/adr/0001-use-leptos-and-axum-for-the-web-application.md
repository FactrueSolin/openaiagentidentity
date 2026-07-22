# Use Leptos and Axum for the web application

The web application will use Axum for its HTTP server and API, with Leptos SSR and browser hydration for a Rust-based frontend. This was chosen over a Yew CSR application and Dioxus Fullstack to provide an integrated Rust full-stack experiment while retaining an explicit HTTP API; deployments must include the generated WASM, JavaScript, and CSS assets alongside the server binary.
