use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose, Engine as _};
use getrandom;
use rand::rngs::OsRng;
use rsa::{
    pkcs8::{DecodePrivateKey, DecodePublicKey},
    Oaep, RsaPrivateKey, RsaPublicKey,
};
use sha2::Sha256;

/// 使用 URL 安全且无填充的 base64 编码字节数据（便于紧凑的 JSON）。
pub fn b64_encode(input: &[u8]) -> String {
    general_purpose::URL_SAFE_NO_PAD.encode(input)
}

/// 解码使用 URL 安全且无填充的 base64 字符串。
pub fn b64_decode(input: &str) -> Result<Vec<u8>, String> {
    general_purpose::URL_SAFE_NO_PAD
        .decode(input)
        .map_err(|e| format!("base64 decode error: {}", e))
}

/// Generate cryptographically secure random bytes.
pub fn random_bytes(len: usize) -> Result<Vec<u8>, String> {
    let mut buf = vec![0u8; len];
    getrandom::getrandom(&mut buf).map_err(|e| format!("random error: {}", e))?;
    Ok(buf)
}

/// Encrypt plaintext with AES-256-GCM, returning (nonce, ciphertext).
pub fn aes_gcm_encrypt(
    key_bytes: &[u8; 32],
    plaintext: &[u8],
) -> Result<(Vec<u8>, Vec<u8>), String> {
    let key = Key::<Aes256Gcm>::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);
    // 96-bit (12 bytes) nonce is standard for GCM
    let nonce_vec = random_bytes(12)?;
    let nonce = Nonce::from_slice(&nonce_vec);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| format!("aes-gcm encrypt error: {}", e))?;
    Ok((nonce_vec, ciphertext))
}

/// Decrypt AES-256-GCM.
pub fn aes_gcm_decrypt(
    key_bytes: &[u8; 32],
    nonce: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, String> {
    let key = Key::<Aes256Gcm>::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(nonce);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("aes-gcm decrypt error: {}", e))?;
    Ok(plaintext)
}

/// Parse a PEM-formatted RSA public key.
pub fn parse_rsa_public_key(pem: &str) -> Result<RsaPublicKey, String> {
    RsaPublicKey::from_public_key_pem(pem).map_err(|e| format!("invalid public key: {}", e))
}

/// Parse a PEM-formatted RSA private key.
pub fn parse_rsa_private_key(pem: &str) -> Result<RsaPrivateKey, String> {
    RsaPrivateKey::from_pkcs8_pem(pem).map_err(|e| format!("invalid private key: {}", e))
}

/// Wrap (encrypt) a symmetric key using RSA-OAEP with SHA-256.
pub fn rsa_oaep_wrap(pub_key: &RsaPublicKey, sym_key: &[u8]) -> Result<Vec<u8>, String> {
    let padding = Oaep::new::<Sha256>();
    let mut rng = OsRng;
    pub_key
        .encrypt(&mut rng, padding, sym_key)
        .map_err(|e| format!("rsa wrap error: {}", e))
}

/// Unwrap (decrypt) a symmetric key using RSA-OAEP with SHA-256.
pub fn rsa_oaep_unwrap(priv_key: &RsaPrivateKey, wrapped: &[u8]) -> Result<Vec<u8>, String> {
    let padding = Oaep::new::<Sha256>();
    priv_key
        .decrypt(padding, wrapped)
        .map_err(|e| format!("rsa unwrap error: {}", e))
}

/// Try reading an environment variable (Node.js) from JS globalThis.process.env.
/// On the browser this will return None.
pub fn read_env_var(name: &str) -> Option<String> {
    use js_sys::Reflect;

    let global = js_sys::global();
    // global.process?.env?.[name]
    let process = Reflect::get(&global, &wasm_bindgen::JsValue::from_str("process")).ok()?;
    let env = Reflect::get(&process, &wasm_bindgen::JsValue::from_str("env")).ok()?;

    // In Node, env is a plain object with string values.
    let val = Reflect::get(&env, &wasm_bindgen::JsValue::from_str(name)).ok()?;
    if val.is_undefined() || val.is_null() {
        None
    } else {
        // Normalize common encodings used in .env: replace literal "\\n" with newline, handle CRLF
        let mut s = val.as_string().unwrap_or_default();
        if s.contains("\\n") || s.contains("\\r") {
            s = s
                .replace("\\r\\n", "\n")
                .replace("\\n", "\n")
                .replace("\\r", "\n");
        }
        // Also normalize any actual CRLF that might be present
        let s = s.replace("\r\n", "\n").replace("\r", "\n");
        Some(s.trim().to_string())
    }
}
