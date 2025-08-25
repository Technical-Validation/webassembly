use wasm_bindgen::prelude::*;

mod utils;
use serde::{Deserialize, Serialize};
use utils::*;
use std::cell::RefCell;
use wasm_bindgen::JsValue;
use js_sys::Date;
use serde_json;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

thread_local! {
    static SESSION: RefCell<Option<SessionState>> = RefCell::new(None);
}

#[derive(Clone)]
struct SessionState {
    key: [u8; 32],
    wrapped_key_b64: String,
    created_ms: u64,
    pubkey_pem: String,
}


fn js_err<E: core::fmt::Display>(e: E) -> JsValue {
    JsValue::from_str(&format!("{}", e))
}

// ===== Session-based AES packet (JSON) and helpers =====
const MAX_AGE_MS: u64 = 15 * 60 * 1000; // 15 minutes

#[derive(Serialize, Deserialize)]
struct AesPacket {
    v: u8,
    sym_alg: String, // "AES-256-GCM"
    nonce_b64: String,
    ciphertext_b64: String,
}

fn now_ms() -> u64 {
    Date::now() as u64
}

fn session_get_if_valid(pubkey_pem: &str) -> Option<SessionState> {
    SESSION.with(|cell| {
        let opt = cell.borrow();
        if let Some(st) = &*opt {
            let not_expired = now_ms().saturating_sub(st.created_ms) <= MAX_AGE_MS;
            if not_expired && st.pubkey_pem == pubkey_pem {
                return Some(st.clone());
            }
        }
        None
    })
}

fn session_get_any() -> Option<SessionState> {
    SESSION.with(|cell| cell.borrow().clone())
}

fn session_set(state: SessionState) {
    SESSION.with(|cell| {
        *cell.borrow_mut() = Some(state);
    })
}

#[wasm_bindgen]
pub fn ensure_session_key(public_key_pem: String) -> Result<String, JsValue> {
    // If there's a valid session for this pubkey, return cached wrapped key
    if let Some(st) = session_get_if_valid(&public_key_pem) {
        let out = serde_json::json!({
            "v": 1,
            "alg": "RSA-OAEP-256",
            "sym_alg": "AES-256-GCM",
            "wrapped_key_b64": st.wrapped_key_b64,
            "fresh": false,
            "created_ms": st.created_ms,
        });
        return Ok(out.to_string());
    }

    // Otherwise, generate a new AES key and wrap it with RSA public key
    let pub_key = parse_rsa_public_key(&public_key_pem).map_err(js_err)?;
    let sym_key = random_bytes(32).map_err(js_err)?;
    if sym_key.len() != 32 { return Err(JsValue::from_str("failed to generate AES-256 key")); }
    let mut key_arr = [0u8; 32];
    key_arr.copy_from_slice(&sym_key);

    let wrapped = rsa_oaep_wrap(&pub_key, &sym_key).map_err(js_err)?;
    let wrapped_b64 = b64_encode(&wrapped);

    let created = now_ms();
    let st = SessionState {
        key: key_arr,
        wrapped_key_b64: wrapped_b64.clone(),
        created_ms: created,
        pubkey_pem: public_key_pem.clone(),
    };
    session_set(st);

    let out = serde_json::json!({
        "v": 1,
        "alg": "RSA-OAEP-256",
        "sym_alg": "AES-256-GCM",
        "wrapped_key_b64": wrapped_b64,
        "fresh": true,
        "created_ms": created,
    });
    Ok(out.to_string())
}

#[wasm_bindgen]
pub fn encrypt_with_session(plaintext_json: String) -> Result<String, JsValue> {
    // Require a non-expired session key
    let st = session_get_any().ok_or_else(|| JsValue::from_str("no session key; call ensure_session_key first"))?;
    let age = now_ms().saturating_sub(st.created_ms);
    if age > MAX_AGE_MS {
        return Err(JsValue::from_str("session key expired; call ensure_session_key to refresh"));
    }

    let (nonce, ciphertext) = aes_gcm_encrypt(&st.key, plaintext_json.as_bytes()).map_err(js_err)?;
    let packet = AesPacket {
        v: 1,
        sym_alg: "AES-256-GCM".to_string(),
        nonce_b64: b64_encode(&nonce),
        ciphertext_b64: b64_encode(&ciphertext),
    };
    serde_json::to_string(&packet).map_err(|e| JsValue::from_str(&format!("serialize error: {}", e)))
}

#[wasm_bindgen]
pub fn decrypt_with_session(packet_json: String) -> Result<String, JsValue> {
    let packet: AesPacket = serde_json::from_str(&packet_json)
        .map_err(|e| JsValue::from_str(&format!("invalid packet json: {}", e)))?;
    if packet.sym_alg != "AES-256-GCM" {
        return Err(JsValue::from_str("unsupported symmetric algorithm"));
    }

    let st = session_get_any().ok_or_else(|| JsValue::from_str("no session key; call ensure_session_key first"))?;

    let nonce = b64_decode(&packet.nonce_b64).map_err(js_err)?;
    let ciphertext = b64_decode(&packet.ciphertext_b64).map_err(js_err)?;
    let plaintext_bytes = aes_gcm_decrypt(&st.key, &nonce, &ciphertext).map_err(js_err)?;
    let plaintext = String::from_utf8(plaintext_bytes)
        .map_err(|_| JsValue::from_str("plaintext is not valid UTF-8"))?;
    Ok(plaintext)
}

fn unwrap_session_key_with_priv(wrapped_key_b64: &str) -> Result<[u8; 32], JsValue> {
    let priv_key_pem = read_env_var("PRIVATE_KEY_PEM")
        .ok_or_else(|| JsValue::from_str("PRIVATE_KEY_PEM not found in env (server-only)"))?;

    let priv_key = parse_rsa_private_key(&priv_key_pem).map_err(js_err)?;
    let wrapped = b64_decode(wrapped_key_b64).map_err(js_err)?;
    let sym_key = rsa_oaep_unwrap(&priv_key, &wrapped).map_err(js_err)?;
    if sym_key.len() != 32 {
        return Err(JsValue::from_str("invalid symmetric key length"));
    }
    let mut key_arr = [0u8; 32];
    key_arr.copy_from_slice(&sym_key);
    Ok(key_arr)
}

#[wasm_bindgen]
pub fn server_decrypt_with_wrapped(wrapped_key_b64: String, packet_json: String) -> Result<String, JsValue> {
    let key = unwrap_session_key_with_priv(&wrapped_key_b64)?;
    let packet: AesPacket = serde_json::from_str(&packet_json)
        .map_err(|e| JsValue::from_str(&format!("invalid packet json: {}", e)))?;
    if packet.sym_alg != "AES-256-GCM" { return Err(JsValue::from_str("unsupported symmetric algorithm")); }
    let nonce = b64_decode(&packet.nonce_b64).map_err(js_err)?;
    let ciphertext = b64_decode(&packet.ciphertext_b64).map_err(js_err)?;
    let plaintext_bytes = aes_gcm_decrypt(&key, &nonce, &ciphertext).map_err(js_err)?;
    let plaintext = String::from_utf8(plaintext_bytes)
        .map_err(|_| JsValue::from_str("plaintext is not valid UTF-8"))?;
    Ok(plaintext)
}

#[wasm_bindgen]
pub fn server_encrypt_with_wrapped(wrapped_key_b64: String, plaintext_json: String) -> Result<String, JsValue> {
    let key = unwrap_session_key_with_priv(&wrapped_key_b64)?;
    let (nonce, ciphertext) = aes_gcm_encrypt(&key, plaintext_json.as_bytes()).map_err(js_err)?;
    let packet = AesPacket {
        v: 1,
        sym_alg: "AES-256-GCM".to_string(),
        nonce_b64: b64_encode(&nonce),
        ciphertext_b64: b64_encode(&ciphertext),
    };
    serde_json::to_string(&packet).map_err(|e| JsValue::from_str(&format!("serialize error: {}", e)))
}
