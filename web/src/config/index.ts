import { env } from "next-runtime-env";

/**
 * Normalize PEM content coming from environment variables.
 * Handles cases where:
 * - Newlines are provided as literal "\n"
 * - Lines use CRLF (\r\n)
 * - Lines are indented or have trailing spaces (e.g., in .env with indentation)
 * - Ensures only the section between BEGIN/END is kept and headers are at column 0
 */
export function normalizePem(input: string | undefined | null): string {
  let s = (input ?? "").trim();
  if (!s) return "";

  // Strip wrapping quotes if present
  if ((s.startsWith('"') && s.endsWith('"')) || (s.startsWith("'") && s.endsWith("'"))) {
    s = s.slice(1, -1);
  }

  // Convert literal \n to actual newlines (safe even if actual newlines exist)
  s = s.replace(/\\n/g, "\n");
  // Normalize CRLF/CR to LF
  s = s.replace(/\r\n/g, "\n").replace(/\r/g, "\n");

  // Split into lines and trim each line to remove indentation/trailing spaces
  const rawLines = s.split("\n").map((l) => l.trim());

  // Keep only the slice between BEGIN ... and END ... boundaries if present
  const beginIdx = rawLines.findIndex((l) => /^-----BEGIN [A-Z0-9 ]+-----$/.test(l));
  const endIdxFrom = beginIdx >= 0 ? beginIdx + 1 : 0;
  const endIdx = rawLines.findIndex((l, i) => i >= endIdxFrom && /^-----END [A-Z0-9 ]+-----$/.test(l));

  let lines: string[] = rawLines.filter((l) => l.length > 0);
  if (beginIdx >= 0 && endIdx >= 0 && endIdx > beginIdx) {
    lines = rawLines.slice(beginIdx, endIdx + 1);
  }

  // Reconstruct with LF and ensure trailing newline (some parsers require it)
  const out = lines.join("\n");
  return out.endsWith("\n") ? out : out + "\n";
}

export const PUBLIC_KEY_PEM = normalizePem(env("NEXT_PUBLIC_PUBLIC_KEY_PEM") || "");
