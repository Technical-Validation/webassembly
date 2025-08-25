# Web + WASM Hybrid Encryption Demo

This app demonstrates a hybrid encryption scheme using a single WASM module shared between client and server.
- Client: encrypts data using a public key via WASM (RSA-OAEP-256 + AES-256-GCM).
- Server: decrypts using the same WASM module. The private key is read EXCLUSIVELY by WASM from environment variables. Node.js code does not read or normalize the private key.

## Requirements
- Node.js 18+
- Rust toolchain + wasm-pack

## Install and Build

1. Build WASM package
```bash
yarn --cwd web build:wasm
```

2. Install dependencies and run dev
```bash
yarn --cwd web yarn-install
yarn --cwd web dev
```

## Environment Variables

We provide a sample env file template:
- web/.env.example (copy to web/.env and fill with your real keys)

Environment variables:
- NEXT_PUBLIC_PUBLIC_KEY_PEM: SPKI public key PEM. Exposed to the browser (safe to expose).
- PRIVATE_KEY_PEM: PKCS#8 private key PEM. Server only. Read solely by WASM.

Important: Do NOT let Node.js code manipulate PRIVATE_KEY_PEM; the WASM module handles line-ending normalization internally.

## Generate a demo RSA keypair (OpenSSL)

```bash
# 1) generate a 2048-bit private key in PKCS#8 format
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out private_pkcs8.pem

# 2) derive the corresponding SPKI public key
openssl pkey -in private_pkcs8.pem -pubout -out public_spki.pem
```

Then copy the contents of these files into web/.env like this (quotes and \n are allowed; code normalizes line endings):

```
NEXT_PUBLIC_PUBLIC_KEY_PEM="-----BEGIN PUBLIC KEY-----\n...base64...\n-----END PUBLIC KEY-----\n"
PRIVATE_KEY_PEM="-----BEGIN PRIVATE KEY-----\n...base64...\n-----END PRIVATE KEY-----\n"
```

## Example PEM Blocks (for reference only; do not use in production)

Public key (SPKI):
```
-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAwXAMPLEPUBLICKEYEXAMP
LEONLYDO_NOT_USE_IN_PRODUCTION_XYZ1234567890abcdefghijklmnopqrstuv
wxyzABCDEFGH1234567890ijklmnopqrstuvwxyzABCDEFGH1234567890abcdefgh
ijklMNOPQRSTuvwxYZ0123456789abcdefghIJ
-----END PUBLIC KEY-----
```

Private key (PKCS#8):
```
-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASC
EXAMPLEPRIVATEKEYEXAMPLEONLY_DO_NOT_USE_IN_PRODUCTION_1234567890ab
cdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789+/=
-----END PRIVATE KEY-----
```

Note: The above blocks are not a working pair. Generate your own with the commands above.

## How it works
- Client reads NEXT_PUBLIC_PUBLIC_KEY_PEM via next-runtime-env and establishes/refreshes an AES session by calling WASM ensure_session_key.
- Client encrypts JSON data using encrypt_with_session and sends { wrapped_key_b64, payload } to the server.
- Server dynamically imports the same WASM module and uses server_decrypt_with_wrapped to decrypt request payload, prepares a response object, then uses server_encrypt_with_wrapped to encrypt and returns { ok, payload }.
- Inside WASM, PRIVATE_KEY_PEM is read from process.env on the server. Node.js code does not access or normalize the private key.

## Lint and Build
```bash
yarn --cwd web lint
yarn --cwd web build
```


## Session-based JSON Protocol (AES session key with 15-minute expiry)

This repository demonstrates a JSON-only interaction with a client-side WASM-managed AES session key.

Key points:
- The client WASM generates a 32-byte AES key and wraps it with the RSA public key using RSA-OAEP-256. Both the raw AES key and its wrapped form (base64url) are cached in a WASM global session state.
- The session has a 15-minute expiry. On each request, the client calls ensure_session_key(publicKeyPem):
  - If a valid, non-expired session exists for the same public key, it reuses the cached AES key and returns { fresh: false }.
  - If expired or the public key changes, it generates a new AES key, wraps it with the provided public key, caches the new key and its wrapped value, and returns { fresh: true }.
- Before encryption on the client, always JSON.stringify your payload; after decryption on the client, JSON.parse the plaintext.
- The server decrypts the wrapped AES key using its PRIVATE_KEY_PEM inside WASM only. For responses, the server encrypts the content using that AES key and returns ONLY the encrypted payload (does not return the AES key).

WASM exports:
- ensure_session_key(public_key_pem: string): string
  - Returns JSON: { v, alg, sym_alg, wrapped_key_b64, fresh, created_ms }
- encrypt_with_session(plaintext_json: string): string
  - Accepts a JSON string (already stringified). Returns AES packet JSON: { v, sym_alg, nonce_b64, ciphertext_b64 }.
- decrypt_with_session(packet_json: string): string
  - Accepts the AES packet JSON string and returns the decrypted plaintext string.
- server_decrypt_with_wrapped(wrapped_key_b64: string, packet_json: string): string
- server_encrypt_with_wrapped(wrapped_key_b64: string, plaintext_json: string): string

API route (POST /api/decrypt):
- Body: { wrapped_key_b64: string, payload: string }
- Behavior: Server uses PRIVATE_KEY_PEM to unwrap the AES key, decrypts incoming payload if needed, prepares a JSON response object, encrypts it with the same AES key, and returns { ok: true, payload } where payload is the AES packet JSON string.

Client demo (page.tsx):
- One consolidated demo: "混合加密 + 会话演示 (全新流程)"
  1) Call ensure_session_key(PUBLIC_KEY_PEM).
  2) Prepare an object and JSON.stringify it.
  3) Call encrypt_with_session to produce the AES packet.
  4) POST { wrapped_key_b64, payload } to /api/decrypt.
  5) Decrypt server payload with decrypt_with_session and JSON.parse the plaintext.

Security:
- All binary fields use URL-safe base64 without padding.
