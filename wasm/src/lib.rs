use wasm_bindgen::prelude::*;

mod utils;
use serde::{Deserialize, Serialize};
use utils::*;

/// Hybrid ciphertext packet. All binary fields are base64url (no padding).
#[derive(Serialize, Deserialize)]
struct HybridPacket {
    /// Version tag for forward compatibility
    v: u8,
    /// Asymmetric algorithm identifier
    alg: String, // e.g., "RSA-OAEP-256"
    /// Symmetric algorithm identifier
    sym_alg: String, // e.g., "AES-256-GCM"
    /// 12-byte GCM nonce
    nonce_b64: String,
    /// RSA-wrapped symmetric key
    wrapped_key_b64: String,
    /// AES-GCM ciphertext (includes tag)
    ciphertext_b64: String,
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen]
pub fn greet(name: &str) {
    // Demonstrate calling browser API from WASM (client side only)
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    // Try using window.alert when in the browser; fall back to console.log otherwise
    if let Some(w) = web_sys::window() {
        let _ = w.alert_with_message(&format!("Hello, {} from WASM!", name));
    } else {
        log(&format!("Hello, {} from WASM (no window detected)", name));
    }
}

/// Encrypt using hybrid scheme (RSA-OAEP-256 + AES-256-GCM).
/// - public_key_pem: PEM string of the RSA public key (SPKI/PKCS#8 public key).
/// - plaintext: UTF-8 plaintext to encrypt.
/// Returns: JSON string of HybridPacket with base64url-encoded fields.
#[wasm_bindgen]
pub fn encrypt_hybrid(public_key_pem: String, plaintext: String) -> Result<String, JsValue> {
    // Parse public key
    let pub_key = parse_rsa_public_key(&public_key_pem).map_err(js_err)?;

    // Generate a random 32-byte AES key
    let sym_key = random_bytes(32).map_err(js_err)?;
    let mut key_arr = [0u8; 32];
    key_arr.copy_from_slice(&sym_key);

    // Encrypt plaintext using AES-256-GCM
    let (nonce, ciphertext) = aes_gcm_encrypt(&key_arr, plaintext.as_bytes()).map_err(js_err)?;

    // Wrap the symmetric key using RSA-OAEP-256
    let wrapped_key = rsa_oaep_wrap(&pub_key, &sym_key).map_err(js_err)?;

    // Build packet
    let packet = HybridPacket {
        v: 1,
        alg: "RSA-OAEP-256".to_string(),
        sym_alg: "AES-256-GCM".to_string(),
        nonce_b64: b64_encode(&nonce),
        wrapped_key_b64: b64_encode(&wrapped_key),
        ciphertext_b64: b64_encode(&ciphertext),
    };

    serde_json::to_string(&packet).map_err(|e| JsValue::from_str(&format!("serialize error: {}", e)))
}

/// Decrypt a hybrid packet using RSA private key fetched from server env.
/// The private key must be provided on the server as env var: PRIVATE_KEY_PEM
/// The same WASM is used by both client and server, but decryption will only
/// succeed on the server where the private key is present in env.
#[wasm_bindgen]
pub fn decrypt_hybrid(packet_json: String) -> Result<String, JsValue> {
    // Parse packet JSON first
    let packet: HybridPacket = serde_json::from_str(&packet_json)
        .map_err(|e| JsValue::from_str(&format!("invalid packet json: {}", e)))?;

    // Ensure algorithms match expected values
    if packet.alg != "RSA-OAEP-256" || packet.sym_alg != "AES-256-GCM" {
        return Err(JsValue::from_str("unsupported algorithms"));
    }

    // Read RSA private key from environment via JS (Node-only)
    let priv_key_pem = read_env_var("PRIVATE_KEY_PEM")
        .ok_or_else(|| JsValue::from_str("PRIVATE_KEY_PEM not found in env (server-only)"))?;

    // Parse private key
    let priv_key = parse_rsa_private_key(&priv_key_pem).map_err(js_err)?;

    // Decode base64 fields
    let nonce = b64_decode(&packet.nonce_b64).map_err(js_err)?;
    let wrapped = b64_decode(&packet.wrapped_key_b64).map_err(js_err)?;
    let ciphertext = b64_decode(&packet.ciphertext_b64).map_err(js_err)?;

    // Unwrap symmetric key
    let sym_key = rsa_oaep_unwrap(&priv_key, &wrapped).map_err(js_err)?;
    if sym_key.len() != 32 {
        return Err(JsValue::from_str("invalid symmetric key length"));
    }
    let mut key_arr = [0u8; 32];
    key_arr.copy_from_slice(&sym_key);

    // Decrypt AES-GCM
    let plaintext_bytes = aes_gcm_decrypt(&key_arr, &nonce, &ciphertext).map_err(js_err)?;
    let plaintext = String::from_utf8(plaintext_bytes)
        .map_err(|_| JsValue::from_str("plaintext is not valid UTF-8"))?;

    Ok(plaintext)
}

fn js_err<E: core::fmt::Display>(e: E) -> JsValue {
    JsValue::from_str(&format!("{}", e))
}
