# Web + WASM 混合加密演示

本应用演示了客户端与服务端共享同一WASM模块的混合加密方案：
- 客户端：通过WASM使用公钥加密数据（RSA-OAEP-256 + AES-256-GCM）
- 服务端：使用相同WASM模块解密。私钥仅由WASM从环境变量读取，Node.js代码不会读取或标准化私钥

## 环境要求
- Node.js 18+
- Rust工具链 + wasm-pack

## 安装与构建

1. 构建WASM包
```bash
yarn --cwd web build:wasm
```

2. 安装依赖并启动开发环境
```bash
yarn --cwd web yarn-install
yarn --cwd web dev
```

## 环境变量配置

提供.env示例模板：
- web/.env.example（复制为web/.env并填写真实密钥）

环境变量说明：
- NEXT_PUBLIC_PUBLIC_KEY_PEM：SPKI格式公钥PEM。暴露在浏览器端（可安全公开）
- PRIVATE_KEY_PEM：PKCS#8格式私钥PEM。仅服务端使用，完全由WASM模块读取

重要提示：禁止让Node.js代码处理PRIVATE_KEY_PEM，WASM模块内部会处理换行符标准化

## 生成演示用RSA密钥对（OpenSSL）

```bash
# 1) 生成2048位PKCS#8格式私钥
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out private_pkcs8.pem

# 2) 导出对应的SPKI格式公钥
openssl pkey -in private_pkcs8.pem -pubout -out public_spki.pem
```

将生成文件内容填入web/.env（允许包含引号和\n，代码会自动标准化换行符）：

```
NEXT_PUBLIC_PUBLIC_KEY_PEM="-----BEGIN PUBLIC KEY-----\n...base64...\n-----END PUBLIC KEY-----\n"
PRIVATE_KEY_PEM="-----BEGIN PRIVATE KEY-----\n...base64...\n-----END PRIVATE KEY-----\n"
```

## PEM示例（仅参考，禁止生产环境使用）

公钥(SPKI):
```
-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAwXAMPLEPUBLICKEYEXAMP
LEONLYDO_NOT_USE_IN_PRODUCTION_XYZ1234567890abcdefghijklmnopqrstuv
wxyzABCDEFGH1234567890ijklmnopqrstuvwxyzABCDEFGH1234567890abcdefgh
ijklMNOPQRSTuvwxYZ0123456789abcdefghIJ
-----END PUBLIC KEY-----
```

私钥(PKCS#8):
```
-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASC
EXAMPLEPRIVATEKEYEXAMPLEONLY_DO_NOT_USE_IN_PRODUCTION_1234567890ab
cdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789+/=
-----END PRIVATE KEY-----
```

注：上述密钥对不可实际使用，请按前文命令生成

## 工作原理
- 客户端通过next-runtime-env读取NEXT_PUBLIC_PUBLIC_KEY_PEM，调用WASM的ensure_session_key建立/刷新AES会话
- 客户端使用encrypt_with_session加密JSON数据，向服务端发送{wrapped_key_b64, payload}
- 服务端动态导入相同WASM模块，使用server_decrypt_with_wrapped解密请求，准备响应对象后用server_encrypt_with_wrapped加密返回{ok, payload}
- WASM内部从process.env读取PRIVATE_KEY_PEM，Node.js代码不接触私钥

## 代码检查与构建
```bash
yarn --cwd web lint
yarn --cwd web build
```

## 基于会话的JSON协议（AES会话密钥15分钟有效期）

本仓库演示纯JSON交互与客户端WASM管理的AES会话密钥机制

核心要点：
- 客户端WASM生成32字节AES密钥，用RSA公钥通过RSA-OAEP-256封装。原始AES密钥及其base64url封装形式缓存在WASM全局会话状态
- 会话有效期15分钟。每次请求时客户端调用ensure_session_key(publicKeyPem)：
  - 若存在相同公钥的有效未过期会话，则复用缓存的AES密钥，返回{fresh: false}
  - 若过期或公钥变更，则生成新AES密钥，用当前公钥封装，缓存新密钥及其封装值，返回{fresh: true}
- 客户端加密前务必对payload进行JSON.stringify，解密后对明文进行JSON.parse
- 服务端在WASM内部使用PRIVATE_KEY_PEM解密封装后的AES密钥。响应时服务端使用该AES密钥加密内容，仅返回加密后的payload（不返回AES密钥）

WASM导出函数：
- ensure_session_key(public_key_pem: string): string
  - 返回JSON：{v, alg, sym_alg, wrapped_key_b64, fresh, created_ms}
- encrypt_with_session(plaintext_json: string): string
  - 接受JSON字符串（需预先stringify），返回AES加密包JSON：{v, sym_alg, nonce_b64, ciphertext_b64}
- decrypt_with_session(packet_json: string): string
  - 接受AES加密包JSON字符串，返回解密后的明文字符串
- server_decrypt_with_wrapped(wrapped_key_b64: string, packet_json: string): string
- server_encrypt_with_wrapped(wrapped_key_b64: string, plaintext_json: string): string

API路由(POST /api/decrypt)：
- 请求体：{wrapped_key_b64: string, payload: string}
- 流程：服务端使用PRIVATE_KEY_PEM解封AES密钥，解密传入payload（如需），准备JSON响应对象后用相同AES密钥加密，返回{ok: true, payload}（payload为AES加密包JSON字符串）

客户端演示(page.tsx)：
- 整合演示："混合加密 + 会话演示（全新流程）"：
  1) 调用ensure_session_key(PUBLIC_KEY_PEM)
  2) 准备对象并进行JSON.stringify
  3) 调用encrypt_with_session生成AES加密包
  4) POST发送{wrapped_key_b64, payload}到/api/decrypt
  5) 用decrypt_with_session解密服务端返回的payload，并对明文字符串进行JSON.parse

安全规范：
- 所有二进制字段均使用无填充的URL安全base64编码