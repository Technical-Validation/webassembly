"use client";
import { PUBLIC_KEY_PEM } from "@/config";

export default function Home() {
  const tools = [
    {
      name: "混合加密 + 会话演示 (全新流程)",
      description:
        "客户端 WASM 维护 AES 会话密钥（15 分钟过期），每次发送 wrapped_key_b64 + 加密 payload；服务端用私钥解包后加密返回，仅返回加密内容。",
      action: () => {
        void (async () => {
          try {
            const mod = (await import("my_wasm_template")) as unknown as {
              default?: () => Promise<void> | void;
              ensure_session_key: (pubKeyPem: string) => string;
              encrypt_with_session: (plaintextJson: string) => string;
              decrypt_with_session: (packetJson: string) => string;
            };
            if (typeof mod.default === "function") {
              await mod.default();
            }

            if (!PUBLIC_KEY_PEM) {
              alert("缺少 NEXT_PUBLIC_PUBLIC_KEY_PEM，无法建立会话。");
              return;
            }

            // 1) 确保/刷新会话密钥（内部若过期会重新生成，并缓存包裹后的密钥）
            const sessStr = mod.ensure_session_key(PUBLIC_KEY_PEM);
            const sess = JSON.parse(sessStr) as { wrapped_key_b64: string; fresh: boolean };

            // 2) 准备 JSON 业务数据，并在加密前 stringify
            const clientObj = { hello: "session aes", clientTime: Date.now() };
            const payloadOut = mod.encrypt_with_session(JSON.stringify(clientObj));

            // 3) 发送到服务端（使用 JSON 协议）
            const resp = await fetch("/api/decrypt", {
              method: "POST",
              headers: { "content-type": "application/json" },
              // 始终携带 wrapped_key_b64，保证服务端无状态
              body: JSON.stringify({ wrapped_key_b64: sess.wrapped_key_b64, payload: payloadOut }),
            });
            const data = await resp.json();
            if (!data?.ok) {
              throw new Error(data?.error || "服务端错误");
            }

            // 4) 客户端使用会话密钥解密服务端返回的加密 payload，并 parse
            const plaintext = mod.decrypt_with_session(data.payload);
            let obj: any;
            try { obj = JSON.parse(plaintext); } catch { obj = { raw: plaintext }; }
            alert(`服务端加密返回: ${JSON.stringify(obj, null, 2)}`);
          } catch (e: unknown) {
            const msg = e instanceof Error ? e.message : typeof e === "string" ? e : JSON.stringify(e);
            alert(`异常3: ${String(msg)}`);
          }
        })();
      },
    },
  ];

  return (
    <div className="min-h-screen bg-gradient-to-br from-gray-50 to-gray-100 p-8 flex flex-col items-center justify-center">
      <div className="max-w-7xl mx-auto text-center w-full">
        <h1 className="text-6xl font-bold text-gray-800 mb-16">
          WebAssembly 示例
        </h1>
        <div className="grid grid-cols-2 gap-8">
          {tools.map((tool, index) => (
            <div
              key={index}
              className="rounded-3xl relative bg-zinc-700 p-10 shadow-2xl hover:shadow-3xl transition-all duration-300 transform hover:-translate-y-2 border border-white/10 backdrop-blur-sm"
            >
              <h2 className="text-4xl font-bold text-white mb-2 whitespace-pre-wrap">
                {tool.name}
              </h2>
              <p className="text-xl text-white whitespace-pre-wrap">
                {tool.description}
              </p>
              <button
                className="mt-8 px-8 py-4 cursor-pointer bg-white bg-opacity-20 rounded-full text-xl font-semibold hover:bg-opacity-30 transition hover:scale-105"
                onClick={tool.action}
              >
                测试
              </button>
              <div className="absolute bottom-4 right-4 text-8xl font-bold text-white/10 z-0">
                {index + 1}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
