use wasm_bindgen::prelude::*;

mod utils;
use serde::{Deserialize, Serialize};
use utils::*;
use std::cell::RefCell;
use wasm_bindgen::JsValue;
use js_sys::Date;
use serde_json;

/// 绑定 JS 的 console.log，用于在 WASM 中调用浏览器/Node 控制台输出。
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

// 线程局部的会话状态存储（WASM 环境下每个线程/实例独立）。
thread_local! {
    static SESSION: RefCell<Option<SessionState>> = RefCell::new(None);
}

#[derive(Clone)]
/// 客户端缓存的会话状态：包含原始 AES 密钥、其 RSA 包裹（base64url）以及创建时间和绑定的公钥 PEM。
struct SessionState {
    /// 32 字节的 AES-256 会话密钥
    key: [u8; 32],
    /// 将会话密钥用 RSA-OAEP-256 包裹后的 base64url（无填充）字符串
    wrapped_key_b64: String,
    /// 会话密钥的创建时间（毫秒），用于判断有效期
    created_ms: u64,
    /// 与该会话绑定的 RSA 公钥（PEM）；若公钥变化则会新建会话
    pubkey_pem: String,
}


/// 将错误格式化为 JsValue，便于通过 wasm_bindgen 返回给 JS 侧。
fn js_err<E: core::fmt::Display>(e: E) -> JsValue {
    JsValue::from_str(&format!("{}", e))
}

// ===== 基于会话的 AES 数据包（JSON）及其辅助方法 =====
/// 会话密钥最长存活时间（毫秒）。默认 15 分钟，过期后需要刷新会话密钥。
const MAX_AGE_MS: u64 = 15 * 60 * 1000; // 15 分钟

#[derive(Serialize, Deserialize)]
/// 通过 AES-256-GCM 传输的加密数据包结构（JSON 序列化）。
struct AesPacket {
    /// 版本号，用于协议演进（当前为 1）
    v: u8,
    /// 对称加密算法名称（固定为 "AES-256-GCM"）
    sym_alg: String,
    /// GCM 使用的随机数（nonce），base64url 无填充编码
    nonce_b64: String,
    /// 密文字节（含认证标签），base64url 无填充编码
    ciphertext_b64: String,
}

/// 获取当前毫秒级时间戳（调用 JS 的 Date::now）。
fn now_ms() -> u64 {
    Date::now() as u64
}

/// 若当前会话未过期且与传入公钥 PEM 匹配，则返回会话状态；否则返回 None。
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

/// 获取当前任意会话状态（不校验是否过期/公钥是否匹配）。
fn session_get_any() -> Option<SessionState> {
    SESSION.with(|cell| cell.borrow().clone())
}

/// 设置/替换当前线程的会话状态。
fn session_set(state: SessionState) {
    SESSION.with(|cell| {
        *cell.borrow_mut() = Some(state);
    })
}

#[wasm_bindgen]
/// 确保存在针对给定公钥 PEM 的 AES 会话密钥；若当前会话有效且公钥一致则复用，否则生成新密钥并返回包裹结果（JSON 字符串）。
pub fn ensure_session_key(public_key_pem: String) -> Result<String, JsValue> {
    // 若该公钥对应的会话仍然有效，直接返回已缓存的包裹会话密钥
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

    // 否则生成新的 AES 会话密钥，并使用 RSA 公钥进行包裹（RSA-OAEP-256）
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
/// 使用当前会话的 AES-256-GCM 加密一个 JSON 字符串（已 stringify），返回 AES 数据包的 JSON 字符串。
pub fn encrypt_with_session(plaintext_json: String) -> Result<String, JsValue> {
    // 需要存在且未过期的会话密钥
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
/// 使用当前会话的 AES-256-GCM 解密从服务端或客户端收到的 AES 数据包（JSON 字符串），返回明文字符串。
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

/// 使用服务器私钥从 wrapped_key_b64 解包出 32 字节 AES 会话密钥（仅服务器使用）。
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
/// 服务器端解密：
/// - 使用 PRIVATE_KEY_PEM 解包 wrapped_key_b64 得到会话 AES 密钥；
/// - 使用该密钥解密传入的 AES 数据包（packet_json，JSON 字符串）；
/// - 返回明文字符串（若不是 UTF-8，将返回错误）。
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
/// 服务器端加密：
/// - 使用 PRIVATE_KEY_PEM 解包 wrapped_key_b64 得到会话 AES 密钥；
/// - 用该密钥加密 plaintext_json（已 stringify 的 JSON 字符串），生成 AES 数据包；
/// - 返回 AES 数据包的 JSON 字符串。
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
