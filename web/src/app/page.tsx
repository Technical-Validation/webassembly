"use client";
import { useState } from "react";
import { PUBLIC_KEY_PEM } from "@/config";

type StepStatus = "idle" | "running" | "done" | "error";
interface StepState {
  status: StepStatus;
  original?: string; // 原始内容（进入该步骤前的内容）
  wasm?: string; // WASM 调用后的内容
  timeMs?: number; // 本步骤耗时
  error?: string;
}

function prettyMaybeJson(input?: string): string | undefined {
  if (typeof input !== "string" || !input.trim()) return input;
  try {
    const parsed = JSON.parse(input);
    return JSON.stringify(parsed, null, 2);
  } catch {
    return input;
  }
}

export default function Home() {
  const [inputJson, setInputJson] = useState<string>(
    JSON.stringify({ hello: "session aes", clientTime: Date.now() }, null, 2)
  );
  const [sending, setSending] = useState(false);

  const [sessionKeyMs, setSessionKeyMs] = useState<number | undefined>(undefined);
  const [sessionFresh, setSessionFresh] = useState<boolean | undefined>(undefined);
  const [wrappedKeyB64, setWrappedKeyB64] = useState<string | undefined>(undefined);

  const [step1, setStep1] = useState<StepState>({ status: "idle" }); // 客户端加密
  const [step2, setStep2] = useState<StepState>({ status: "idle" }); // 服务端解密
  const [step3, setStep3] = useState<StepState>({ status: "idle" }); // 服务端对响应加密
  const [step4, setStep4] = useState<StepState>({ status: "idle" }); // 客户端解密

  function resetSteps() {
    setStep1({ status: "idle" });
    setStep2({ status: "idle" });
    setStep3({ status: "idle" });
    setStep4({ status: "idle" });
    setSessionKeyMs(undefined);
    setSessionFresh(undefined);
    setWrappedKeyB64(undefined);
  }

  async function onSend() {
    if (sending) return;
    resetSteps();
    setSending(true);

    try {
      // 校验 JSON
      let parsed: unknown;
      try {
        parsed = JSON.parse(inputJson);
      } catch (e) {
        setStep1({ status: "error", error: "输入不是合法 JSON" });
        setSending(false);
        return;
      }

      // 加载 WASM 模块
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
        setStep1({ status: "error", error: "缺少 NEXT_PUBLIC_PUBLIC_KEY_PEM，无法建立会话。" });
        setSending(false);
        return;
      }

      // 会话密钥（仅 TS 侧计时，不写入明文）
      const tSess0 = performance.now?.() ?? Date.now();
      const sessStr = mod.ensure_session_key(PUBLIC_KEY_PEM);
      const tSess1 = performance.now?.() ?? Date.now();
      setSessionKeyMs(tSess1 - tSess0);
      const sess = JSON.parse(sessStr) as { wrapped_key_b64: string; fresh: boolean };
      setSessionFresh(sess.fresh);
      setWrappedKeyB64(sess.wrapped_key_b64);

      // Step 1: 客户端加密
      setStep1({ status: "running", original: JSON.stringify(parsed, null, 2) });
      const tCliEnc0 = performance.now?.() ?? Date.now();
      const requestCipher = mod.encrypt_with_session(JSON.stringify(parsed));
      const tCliEnc1 = performance.now?.() ?? Date.now();
      setStep1({ status: "done", original: JSON.stringify(parsed, null, 2), wasm: requestCipher, timeMs: tCliEnc1 - tCliEnc0 });

      // 调用服务端
      const resp = await fetch("/api/decrypt", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ wrapped_key_b64: sess.wrapped_key_b64, payload: requestCipher }),
      });
      const data = await resp.json();
      if (!data?.ok) {
        const errMsg = data?.error || "服务端错误";
        setStep2({ status: "error", error: errMsg, original: requestCipher });
        setSending(false);
        return;
      }

      // Step 2: 服务端解密（展示：原始=请求密文；WASM=服务端解出的明文）
      setStep2({
        status: "done",
        original: requestCipher,
        wasm: typeof data?.debug?.server_decrypted_plaintext === "string" ? data.debug.server_decrypted_plaintext : "",
        timeMs: data?.timings?.server_decrypt_ms,
      });

      // Step 3: 服务端对响应加密（展示：原始=服务端响应明文；WASM=响应密文）
      setStep3({
        status: "done",
        original: typeof data?.debug?.server_response_plaintext === "string" ? data.debug.server_response_plaintext : "",
        wasm: data.payload,
        timeMs: data?.timings?.server_encrypt_ms,
      });

      // Step 4: 客户端解密
      setStep4({ status: "running", original: data.payload });
      const tCliDec0 = performance.now?.() ?? Date.now();
      const responsePlain = mod.decrypt_with_session(data.payload);
      const tCliDec1 = performance.now?.() ?? Date.now();
      setStep4({ status: "done", original: data.payload, wasm: responsePlain, timeMs: tCliDec1 - tCliDec0 });
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : typeof e === "string" ? e : JSON.stringify(e);
      setStep1((s) => (s.status === "idle" ? { status: "error", error: String(msg) } : s));
    } finally {
      setSending(false);
    }
  }

  function StepCard({ title, step }: { title: string; step: StepState }) {
    return (
      <div className="rounded-xl border border-gray-200 bg-white p-4 shadow-sm">
        <div className="flex items-center justify-between mb-2">
          <h3 className="text-lg font-semibold text-gray-800">{title}</h3>
          <span className="text-xs px-2 py-0.5 rounded-full border" data-status={step.status}>
            {step.status === "idle" && "未开始"}
            {step.status === "running" && "执行中"}
            {step.status === "done" && "已完成"}
            {step.status === "error" && "出错"}
          </span>
        </div>
        {step.error && <div className="text-red-600 text-sm mb-2">{step.error}</div>}
        {step.original && (
          <div className="mb-2">
            <div className="text-xs text-gray-500 mb-1">原始内容</div>
            <pre className="text-xs bg-gray-50 p-2 rounded overflow-auto max-h-40 whitespace-pre-wrap break-words">{prettyMaybeJson(step.original)}</pre>
          </div>
        )}
        {step.wasm && (
          <div className="mb-2">
            <div className="text-xs text-gray-500 mb-1">WASM 内容</div>
            <pre className="text-xs bg-gray-50 p-2 rounded overflow-auto max-h-40 whitespace-pre-wrap break-words">{prettyMaybeJson(step.wasm)}</pre>
          </div>
        )}
        {typeof step.timeMs === "number" && (
          <div className="text-xs text-gray-600">耗时: {step.timeMs.toFixed(2)} ms</div>
        )}
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-gradient-to-br from-gray-50 to-gray-100 p-6">
      <div className="max-w-5xl mx-auto space-y-6">
        <h1 className="text-3xl font-bold text-gray-800">混合加密 + 会话演示 · 测试页面</h1>

        <div className="rounded-xl border border-gray-200 bg-white p-4 shadow-sm">
          <label className="block text-sm font-medium text-gray-700 mb-2">输入 JSON</label>
          <textarea
            className="w-full h-40 p-3 border rounded font-mono text-xs"
            value={inputJson}
            onChange={(e) => setInputJson(e.target.value)}
            disabled={sending}
          />
          <div className="mt-3 flex items-center gap-3">
            <button
              className="px-4 py-2 bg-zinc-800 text-white rounded disabled:opacity-50"
              onClick={onSend}
              disabled={sending}
            >
              {sending ? "发送中..." : "发送"}
            </button>
            {typeof sessionKeyMs === "number" && (
              <div className="text-xs text-gray-600">
                会话密钥耗时: {sessionKeyMs.toFixed(2)} ms {sessionFresh !== undefined && `(${sessionFresh ? "新建" : "复用"})`}
              </div>
            )}
            {wrappedKeyB64 && (
              <div className="text-xs text-gray-500 truncate max-w-[50%]" title={wrappedKeyB64}>
                wrapped_key_b64: {wrappedKeyB64}
              </div>
            )}
          </div>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <StepCard title="步骤 1：客户端加密" step={step1} />
          <StepCard title="步骤 2：服务端解密" step={step2} />
          <StepCard title="步骤 3：服务端对响应加密" step={step3} />
          <StepCard title="步骤 4：客户端解密" step={step4} />
        </div>
      </div>
    </div>
  );
}
