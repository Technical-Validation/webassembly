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

/// 生成加密安全的随机字节（使用操作系统熵源）。
pub fn random_bytes(len: usize) -> Result<Vec<u8>, String> {
    let mut buf = vec![0u8; len];
    getrandom::getrandom(&mut buf).map_err(|e| format!("random error: {}", e))?;
    Ok(buf)
}

/// 使用 AES-256-GCM 加密明文，返回 (nonce, ciphertext)。
pub fn aes_gcm_encrypt(
    key_bytes: &[u8; 32],
    plaintext: &[u8],
) -> Result<(Vec<u8>, Vec<u8>), String> {
    let key = Key::<Aes256Gcm>::from_slice(key_bytes);
    let cipher = Aes256Gcm::new(key);
    // GCM 标准推荐使用 96 位（12 字节）随机随机数（nonce）
    let nonce_vec = random_bytes(12)?;
    let nonce = Nonce::from_slice(&nonce_vec);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| format!("aes-gcm encrypt error: {}", e))?;
    Ok((nonce_vec, ciphertext))
}

/// 使用 AES-256-GCM 解密密文，返回明文字节。
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

/// 解析 PEM 格式的 RSA 公钥（SPKI）。
pub fn parse_rsa_public_key(pem: &str) -> Result<RsaPublicKey, String> {
    RsaPublicKey::from_public_key_pem(pem).map_err(|e| format!("invalid public key: {}", e))
}

/// 解析 PEM 格式的 RSA 私钥（PKCS#8）。
pub fn parse_rsa_private_key(pem: &str) -> Result<RsaPrivateKey, String> {
    RsaPrivateKey::from_pkcs8_pem(pem)
        .map_err(|e8| format!("Invalid private key: PKCS#8 parse error: {}", e8))
}

/// 使用 RSA-OAEP（SHA-256）包裹（加密）对称密钥。
pub fn rsa_oaep_wrap(pub_key: &RsaPublicKey, sym_key: &[u8]) -> Result<Vec<u8>, String> {
    let padding = Oaep::new::<Sha256>();
    let mut rng = OsRng;
    pub_key
        .encrypt(&mut rng, padding, sym_key)
        .map_err(|e| format!("rsa wrap error: {}", e))
}

/// 使用 RSA-OAEP（SHA-256）解包（解密）对称密钥。
pub fn rsa_oaep_unwrap(priv_key: &RsaPrivateKey, wrapped: &[u8]) -> Result<Vec<u8>, String> {
    let padding = Oaep::new::<Sha256>();
    priv_key
        .decrypt(padding, wrapped)
        .map_err(|e| format!("rsa unwrap error: {}", e))
}

/// 从 JS 的 globalThis.process.env 读取环境变量（Node.js 环境可用）。
/// 在浏览器环境下不可用，函数会返回 None。
pub fn read_env_var(name: &str) -> Option<String> {
    use js_sys::Reflect;

    let global = js_sys::global();
    // 访问 global.process?.env?.[name]
    let process = Reflect::get(&global, &wasm_bindgen::JsValue::from_str("process")).ok()?;
    let env = Reflect::get(&process, &wasm_bindgen::JsValue::from_str("env")).ok()?;

    // 在 Node 中，env 是一个键为字符串、值为字符串的普通对象
    let val = Reflect::get(&env, &wasm_bindgen::JsValue::from_str(name)).ok()?;
    if val.is_undefined() || val.is_null() {
        None
    } else {
        // 将值读取为字符串
        let mut s = val.as_string().unwrap_or_default();

        // 若存在 UTF-8 BOM，移除之
        s = s.trim_start_matches('\u{feff}').to_string();

        // 若存在包裹引号，则去除首尾引号
        if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
            s = s[1..s.len().saturating_sub(1)].to_string();
        }

        // 将字面量转义序列（\\n/\\r）转换为真实换行
        if s.contains("\\n") || s.contains("\\r") {
            s = s
                .replace("\\r\\n", "\n")
                .replace("\\n", "\n")
                .replace("\\r", "\n");
        }
        // 将任何实际存在的 CRLF/CR 统一为 LF
        s = s.replace("\r\n", "\n").replace("\r", "\n");

        // 拆分为多行，逐行 trim；若检测到 BEGIN..END 边界，仅保留其间内容
        let raw_lines: Vec<String> = s
            .split('\n')
            .map(|l| l.trim().to_string())
            .collect();

        let mut begin_idx: Option<usize> = None;
        let mut end_idx: Option<usize> = None;
        for (i, l) in raw_lines.iter().enumerate() {
            if begin_idx.is_none() && l.starts_with("-----BEGIN ") && l.ends_with("-----") {
                begin_idx = Some(i);
            } else if begin_idx.is_some() && l.starts_with("-----END ") && l.ends_with("-----") {
                end_idx = Some(i);
                break;
            }
        }

        let lines: Vec<String> = if let (Some(b), Some(e)) = (begin_idx, end_idx) {
            raw_lines[b..=e].to_vec()
        } else {
            raw_lines.into_iter().filter(|l| !l.is_empty()).collect()
        };

        // 重建字符串并确保以换行结尾（部分解析器需要）
        let mut out = lines.join("\n");
        if !out.ends_with('\n') {
            out.push('\n');
        }
        Some(out)
    }
}
