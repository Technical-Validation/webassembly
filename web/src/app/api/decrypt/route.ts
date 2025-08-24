import { NextRequest } from "next/server";
// Ensure this route runs in Node.js so process.env is available for WASM
export const runtime = "nodejs";
// Import the same WASM module on the server via dynamic import below.
// The WASM will read PRIVATE_KEY_PEM from process.env when decrypt_hybrid is called.

export async function POST(req: NextRequest) {
  try {
    // Expect JSON body of shape { packet: string }
    const body = await req.json();
    const packet: string = body?.packet;
    if (typeof packet !== "string") {
      return new Response(JSON.stringify({ error: "missing packet" }), {
        status: 400,
        headers: { "content-type": "application/json" },
      });
    }

    // 动态导入 WASM 并解密。WASM 会从 env 读取 PRIVATE_KEY_PEM。
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
    // If PRIVATE_KEY_PEM is missing or decrypt fails, we surface the error.
    const message = err instanceof Error ? err.message : typeof err === "string" ? err : JSON.stringify(err);
    return new Response(
      JSON.stringify({ ok: false, error: String(message) }),
      { status: 500, headers: { "content-type": "application/json" } },
    );
  }
}
