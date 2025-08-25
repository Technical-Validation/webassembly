use wasm_bindgen::prelude::*;

mod utils;
use serde::{Deserialize, Serialize};
use utils::*;

/// 混合密文包。所有二进制字段使用 base64url 编码（无填充）。
#[derive(Serialize, Deserialize)]
struct HybridPacket {
    /// 版本标签，用于向前兼容
    v: u8,
    /// 非对称算法标识符
    alg: String, // 例如 "RSA-OAEP-256"
    /// 对称算法标识符
    sym_alg: String, // 例如 "AES-256-GCM"
    /// 12 字节 GCM 随机数（nonce）
    nonce_b64: String,
    /// 用 RSA 包裹的对称密钥
    wrapped_key_b64: String,
    /// AES-GCM 密文（包含认证标签 tag ）
    ciphertext_b64: String,
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[wasm_bindgen]
pub fn greet(name: &str) {
    // 演示从 WASM 调用浏览器 API（仅在客户端生效）
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    // 在浏览器中尝试使用 window.alert；否则退回 console.log
    if let Some(w) = web_sys::window() {
        let _ = w.alert_with_message(&format!("Hello, {} from WASM!", name));
    } else {
        log(&format!("Hello, {} from WASM (no window detected)", name));
    }
}

/// 使用混合加密方案（RSA-OAEP-256 + AES-256-GCM）。
/// - public_key_pem：RSA 公钥的 PEM 字符串（SPKI/PKCS#8 公钥）。
/// - plaintext：需要加密的 UTF-8 明文。
/// 返回：包含 base64url 字段的 HybridPacket JSON 字符串。
#[wasm_bindgen]
pub fn encrypt_hybrid(public_key_pem: String, plaintext: String) -> Result<String, JsValue> {
    // 解析公钥
    let pub_key = parse_rsa_public_key(&public_key_pem).map_err(js_err)?;

    // 生成 32 字节随机 AES 密钥
    let sym_key = random_bytes(32).map_err(js_err)?;
    let mut key_arr = [0u8; 32];
    key_arr.copy_from_slice(&sym_key);

    // 使用 AES-256-GCM 加密明文
    let (nonce, ciphertext) = aes_gcm_encrypt(&key_arr, plaintext.as_bytes()).map_err(js_err)?;

    // 使用 RSA-OAEP-256 包裹（加密）对称密钥
    let wrapped_key = rsa_oaep_wrap(&pub_key, &sym_key).map_err(js_err)?;

    // 构造数据包
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

/// 使用服务端环境变量中的 RSA 私钥解密混合密文包。
/// 服务器需通过环境变量提供私钥：PRIVATE_KEY_PEM
/// 客户端与服务端共享同一 WASM 模块，但只有在服务端（存在私钥的环境）
/// 解密才会成功。
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
