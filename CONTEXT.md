# Agent Identity Registration

This context covers creating a reusable OpenAI Agent Identity from an authorized ChatGPT access token.

## Language

**Access Token**:
A short-lived ChatGPT JWT supplied by an authorized user to register an Agent Runtime. It is sensitive input and is not part of the resulting identity.
_Avoid_: Auth file, identity token

**Agent Runtime**:
An OpenAI-registered runtime identified by the Runtime ID returned after its public key is accepted.
_Avoid_: Agent account, session

**Key Material**:
The Ed25519 key pair that binds an Agent Identity to its Agent Runtime. Its private key is secret while its public key is submitted during registration.
_Avoid_: Token, password

**Agent Identity**:
The reusable credential document containing an Agent Runtime ID, its private key, and associated ChatGPT account metadata.
_Avoid_: Auth JSON, authentication file, token file
