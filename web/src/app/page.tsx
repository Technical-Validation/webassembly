"use client";
import { env } from "next-runtime-env";

export default function Home() {
  const tools = [
    {
      name: "wasm 调用浏览器 api",
      description: 'wasm.greet("browser") => alert("")',
      action: () => {
        void (async () => {
          try {
            const mod = (await import("my_wasm_template")) as unknown as {
              default?: () => Promise<void> | void;
              greet: (name: string) => void;
            };
            if (typeof mod.default === "function") {
              await mod.default();
            }
            mod.greet("browser");
          } catch (e: unknown) {
            const msg = e instanceof Error ? e.message : typeof e === "string" ? e : JSON.stringify(e);
            alert(`异常: ${String(msg)}`);
          }
        })();
      },
    },
    {
      name: "混合加密演示 (客户端加密 -> 服务端解密)",
      description:
        "读取 NEXT_PUBLIC_PUBLIC_KEY_PEM 公钥，在浏览器端用 WASM 进行混合加密，发送到 /api/decrypt 由服务端同一 WASM 模块解密",
      action: () => {
        void (async () => {
          try {
            // 动态导入 WASM 模块并初始化（兼容 async WebAssembly 打包）
            const mod = (await import("my_wasm_template")) as unknown as {
              default?: () => Promise<void> | void;
              encrypt_hybrid: (pubKeyPem: string, msg: string) => string;
            };
            if (typeof mod.default === "function") {
              await mod.default();
            }

            // 通过 next-runtime-env 从运行时环境读取公钥（只暴露给客户端的 NEXT_PUBLIC_ 变量）
            const rawPublicKeyPem = env("NEXT_PUBLIC_PUBLIC_KEY_PEM");
            if (!rawPublicKeyPem) {
              alert("缺少 NEXT_PUBLIC_PUBLIC_KEY_PEM，无法加密。请在环境变量中设置公钥。");
              return;
            }
            // 规范化 PEM：支持 .env 中用 \n 表示换行，以及实际的 CRLF 行尾
            const publicKeyPem = rawPublicKeyPem
              .replace(/\\r\\n/g, "\n")
              .replace(/\\n/g, "\n")
              .replace(/\\r/g, "\n")
              .replace(/\r\n/g, "\n")
              .replace(/\r/g, "\n")
              .trim();

            const message = "Hello Hybrid Crypto via WASM!";

            // 使用相同的 WASM 模块在客户端执行混合加密（RSA-OAEP-256 + AES-256-GCM）
            const packetJson = mod.encrypt_hybrid(publicKeyPem, message);

            // 发送到服务端 API，由服务端用相同 WASM 模块解密（WASM 内部从 env 读取 PRIVATE_KEY_PEM）
            const resp = await fetch("/api/decrypt", {
              method: "POST",
              headers: { "content-type": "application/json" },
              body: JSON.stringify({ packet: packetJson }),
            });
            const data = await resp.json();
            if (data?.ok) {
              alert(`服务端解密成功: ${data.plaintext}`);
            } else {
              alert(`服务端解密失败: ${data?.error ?? "未知错误"}`);
            }
          } catch (e: unknown) {
            const msg = e instanceof Error ? e.message : typeof e === "string" ? e : JSON.stringify(e);
            alert(`异常: ${String(msg)}`);
          }
        })();
      },
    },
  ];

  return (
    <div className="min-h-screen bg-gradient-to-br from-gray-50 to-gray-100 p-8 flex flex-col items-center justify-center">
      <div className="max-w-7xl mx-auto text-cente w-full">
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
