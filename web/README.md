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
- Client page reads NEXT_PUBLIC_PUBLIC_KEY_PEM via next-runtime-env and calls WASM encrypt_hybrid.
- The server API route dynamically imports the same WASM module and calls decrypt_hybrid; inside WASM, PRIVATE_KEY_PEM is read from process.env via JS interop. Node code does NOT access the key.

## Lint and Build
```bash
yarn --cwd web lint
yarn --cwd web build
```
