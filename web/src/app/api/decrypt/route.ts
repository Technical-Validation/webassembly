import { NextRequest } from "next/server";
import { normalizePem } from "@/config";
// 确保该路由在 Node.js 运行，以便 WASM 可以访问 process.env
export const runtime = "nodejs";
// 通过动态导入在服务端加载相同的 WASM 模块
// 在调用 decrypt_hybrid 时，WASM 会从 process.env 读取 PRIVATE_KEY_PEM

export async function POST(req: NextRequest) {
  try {
    // 期望请求体为 JSON，形如 { packet: string }
    const body = await req.json();
    const packet: string = body?.packet;
    if (typeof packet !== "string") {
      return new Response(JSON.stringify({ error: "缺少 packet" }), {
        status: 400,
        headers: { "content-type": "application/json" },
      });
    }

    // 动态导入 WASM 并解密。WASM 会从 env 读取 PRIVATE_KEY_PEM。
    // 在调用 WASM 之前，规范化 PRIVATE_KEY_PEM，避免换行与缩进导致的 PEM 解析错误
    const normalized = normalizePem(process.env.PRIVATE_KEY_PEM || "");
    if (normalized) {
      process.env.PRIVATE_KEY_PEM = normalized;
    }
    const mod = (await import("my_wasm_template")) as unknown as {
      default?: () => Promise<void> | void;
      decrypt_hybrid: (packet: string) => string;
    };
    if (typeof mod.default === "function") {
      await mod.default();
    }
    const plaintext = mod.decrypt_hybrid(packet);

    return new Response(
      JSON.stringify({ ok: true, plaintext }),
      { status: 200, headers: { "content-type": "application/json" } },
    );
  } catch (err: unknown) {
    // 若缺少 PRIVATE_KEY_PEM 或解密失败，则返回错误信息
    const message = err instanceof Error ? err.message : typeof err === "string" ? err : JSON.stringify(err);
    return new Response(
      JSON.stringify({ ok: false, error: String(message) }),
      { status: 500, headers: { "content-type": "application/json" } },
    );
  }
}
