import { NextRequest } from "next/server";
// 确保该路由在 Node.js 运行，以便 WASM 可以访问 process.env
export const runtime = "nodejs";
// 动态导入在服务端加载相同的 WASM 模块

/**
 * POST /api/decrypt
 * 服务器端处理加密通信的入口。
 * 入参:
 * - 请求体 JSON: { wrapped_key_b64: string, payload: string }
 * 处理:
 * - 动态加载同一 WASM 模块，使用服务器私钥解包会话密钥；
 * - 解密客户端 payload，构造响应对象，再用相同会话密钥加密返回。
 * 返回:
 * - 200: { ok: true, payload: string } 其中 payload 为 AES 数据包 JSON 字符串
 * - 4xx/5xx: { ok: false, error: string }
 */
export async function POST(req: NextRequest) {
  try {
    const body = await req.json();

    // 延迟加载 WASM（WASM 内部自行从 env 读取并规范化 PRIVATE_KEY_PEM）
    const mod = (await import("my_wasm_template")) as unknown as {
      default?: () => Promise<void> | void;
      server_decrypt_with_wrapped: (wrappedKeyB64: string, packet: string) => string;
      server_encrypt_with_wrapped: (wrappedKeyB64: string, plaintextJson: string) => string;
    };
    if (typeof mod.default === "function") {
      await mod.default();
    }

    // 仅支持全新 JSON 协议：{ wrapped_key_b64: string, payload: string }
    const wrappedKeyB64 = body?.wrapped_key_b64;
    const payload = body?.payload;
    if (typeof wrappedKeyB64 !== "string" || typeof payload !== "string") {
      return new Response(JSON.stringify({ ok: false, error: "缺少 wrapped_key_b64 或 payload" }), {
        status: 400,
        headers: { "content-type": "application/json" },
      });
    }

    // 使用私钥解出 AES 会话密钥后，解密客户端 payload（可选：用于读取业务字段）
    let incomingPlaintext = "";
    const tServerDec0 = Date.now();
    try {
      incomingPlaintext = mod.server_decrypt_with_wrapped(wrappedKeyB64, payload);
    } catch (e) {
      throw e;
    }
    const server_decrypt_ms = Date.now() - tServerDec0;

    // 示例业务：回显 + 服务器时间戳
    let clientObj: unknown = undefined;
    try { clientObj = JSON.parse(incomingPlaintext); } catch {}
    const responseObj = {
      echo: clientObj ?? incomingPlaintext,
      serverTime: Date.now(),
      msg: "server encrypted response",
    };
    const responseJson = JSON.stringify(responseObj);

    // 使用相同 AES 会话密钥加密返回内容
    const tServerEnc0 = Date.now();
    const outPacket = mod.server_encrypt_with_wrapped(wrappedKeyB64, responseJson);
    const server_encrypt_ms = Date.now() - tServerEnc0;

    const debug = {
      server_decrypted_plaintext: incomingPlaintext,
      server_response_plaintext: responseJson,
    };

    return new Response(
      JSON.stringify({ ok: true, payload: outPacket, timings: { server_decrypt_ms, server_encrypt_ms }, debug }),
      { status: 200, headers: { "content-type": "application/json" } },
    );
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : typeof err === "string" ? err : JSON.stringify(err);
    return new Response(
      JSON.stringify({ ok: false, error: String(message) }),
      { status: 500, headers: { "content-type": "application/json" } },
    );
  }
}
