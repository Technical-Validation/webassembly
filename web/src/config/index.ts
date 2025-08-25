import { env } from "next-runtime-env";

/**
 * 规范化来自环境变量的 PEM 文本。
 * 处理以下常见情况：
 * - 换行以字面量 "\n" 提供
 * - 行使用 CRLF (\r\n)
 * - 行存在缩进或尾随空格（例如 .env 中缩进导致）
 * - 仅保留 BEGIN/END 之间的主体内容，并确保头尾位于第 0 列
 */
export function normalizePem(input: string | undefined | null): string {
  let s = (input ?? "").trim();
  if (!s) return "";

  // 如果存在包裹的引号，则去除
  if ((s.startsWith('"') && s.endsWith('"')) || (s.startsWith("'") && s.endsWith("'"))) {
    s = s.slice(1, -1);
  }

  // 将字面量 "\n" 转换为实际换行（即使原本已存在也安全）
  s = s.replace(/\\n/g, "\n");
  // 将 CRLF/CR 统一规范化为 LF
  s = s.replace(/\r\n/g, "\n").replace(/\r/g, "\n");

  // 按行拆分并去除每行首尾空白，以清除缩进/尾随空格
  const rawLines = s.split("\n").map((l) => l.trim());

  // 如存在 BEGIN ... 与 END ... 边界，仅保留其间内容
  const beginIdx = rawLines.findIndex((l) => /^-----BEGIN [A-Z0-9 ]+-----$/.test(l));
  const endIdxFrom = beginIdx >= 0 ? beginIdx + 1 : 0;
  const endIdx = rawLines.findIndex((l, i) => i >= endIdxFrom && /^-----END [A-Z0-9 ]+-----$/.test(l));

  let lines: string[] = rawLines.filter((l) => l.length > 0);
  if (beginIdx >= 0 && endIdx >= 0 && endIdx > beginIdx) {
    lines = rawLines.slice(beginIdx, endIdx + 1);
  }

  // 使用 LF 重建并确保末尾换行（部分解析器需要）
  const out = lines.join("\n");
  return out.endsWith("\n") ? out : out + "\n";
}

export const PUBLIC_KEY_PEM = normalizePem(env("NEXT_PUBLIC_PUBLIC_KEY_PEM") || "");
