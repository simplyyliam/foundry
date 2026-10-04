// Rust's `char::is_whitespace` (Unicode White_Space), which the backend's `normalize_ws`
// splits on. JS `\s` differs: it adds U+FEFF and lacks U+0085.
const WHITESPACE = /[\t\n\v\f\r \u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]+/;

/** Runs of whitespace become one space; trimmed. Same as the backend's `normalize_ws`. */
export function normalizeWhitespace(text: string): string {
  return text.split(WHITESPACE).filter(Boolean).join(" ");
}
