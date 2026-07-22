# Offer the web tool without user authentication

The web application will be a publicly accessible online tool without its own login system, prioritizing immediate use over account management. Production deployments must terminate HTTPS before traffic reaches Axum, and neither that infrastructure nor the application may persist or log Access Tokens or generated Agent Identities; abuse controls may be added separately if operational evidence requires them.
