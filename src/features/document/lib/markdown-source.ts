import type { MarkdownManager } from "@tiptap/markdown";
import { Node, type Schema } from "@tiptap/pm/model";

/**
 * Ties the editor's top-level blocks to the exact markdown they came from, so a save only
 * re-serializes the blocks that were edited and copies every other block's original text
 * byte for byte. The serializer otherwise rewrites untouched text (`_a_` → `*a*`,
 * `<div>` → `&lt;div&gt;`, `[x]` → `\[x\]`).
 *
 * ProseMirror reuses unchanged nodes, so "unchanged" is node identity: an untouched block is
 * the same object and keeps its text. A new node equal (`eq`) to an original block also keeps
 * that block's text; anything else is serialized.
 */

interface Span {
  start: number;
  end: number;
}

export interface SourceMap {
  /** The markdown on disk, with `\n` line endings. */
  source: string;
  /** Line ending to write back (the file's own). */
  eol: "\n" | "\r\n";
  /** The top-level blocks `source` was read into (or written from). */
  nodes: readonly Node[];
  /**
   * Each node's text in `source`. Null for a blank line kept as an empty paragraph, or a
   * block whose text didn't verifiably parse back to it.
   */
  spans: readonly (Span | null)[];
  /**
   * The text of every block verified at load, by type and text, kept across saves: a block
   * edited and then restored (undo) is written back exactly as it was in the file.
   */
  loaded: ReadonlyMap<string, readonly { node: Node; text: string }[]>;
}

const textKey = (node: Node) => `${node.type.name}\u0000${node.textContent}`;

const isEmptyParagraph = (node: Node) => node.type.name === "paragraph" && node.childCount === 0;

const trimTrailingNewlines = (text: string) => text.replace(/\n+$/, "");

/**
 * Lines the loaded document up with its source. Each top-level markdown token is parsed on
 * its own and must produce exactly the block at that position (`eq`), or no block keeps its
 * original text: the whole file is then serialized on save, as the library would.
 */
export function createSourceMap(raw: string, doc: Node, manager: MarkdownManager, schema: Schema): SourceMap {
  const source = raw.replace(/\r\n?/g, "\n");
  const eol = raw.includes("\r\n") ? "\r\n" : "\n";
  const nodes: Node[] = [];
  doc.forEach((child) => nodes.push(child));
  const spans = aligned(source, nodes, manager, schema) ?? nodes.map(() => null);
  const loaded = new Map<string, { node: Node; text: string }[]>();
  nodes.forEach((node, i) => {
    const span = spans[i];
    if (!span) return;
    const key = textKey(node);
    loaded.set(key, [...(loaded.get(key) ?? []), { node, text: source.slice(span.start, span.end) }]);
  });
  return { source, eol, nodes, spans, loaded };
}

function aligned(source: string, nodes: Node[], manager: MarkdownManager, schema: Schema): Span[] | null {
  const spans: (Span | null)[] = nodes.map(() => null);
  let offset = 0;
  let index = 0;
  try {
    for (const token of manager.instance.lexer(source)) {
      const start = offset;
      offset += token.raw.length;
      if (token.type === "space") continue;

      // A token's raw text can end with the blank lines after it; those aren't part of the block.
      const text = trimTrailingNewlines(token.raw);
      const parsed = Node.fromJSON(schema, manager.parse(text));
      if (parsed.childCount === 0) continue; // e.g. a link definition: kept only between untouched blocks
      if (parsed.childCount !== 1) return null;
      const block = parsed.child(0);
      // Blank lines the parser turned into empty paragraphs have no token of their own.
      while (index < nodes.length && isEmptyParagraph(nodes[index]) && !isEmptyParagraph(block)) index += 1;
      if (index >= nodes.length || nodes[index].type !== block.type) return null;

      // Parsed alone it can differ (a `[x][1]` link whose definition is elsewhere): that
      // block is then serialized like an edited one.
      if (nodes[index].eq(block)) spans[index] = { start, end: start + text.length };
      index += 1;
    }
  } catch {
    return null;
  }
  if (offset !== source.length) return null;
  if (nodes.slice(index).some((node) => !isEmptyParagraph(node))) return null;
  return spans as Span[];
}

/**
 * The markdown to save for `doc`, and the source map describing it (adopt it once the save
 * succeeds). Untouched runs of blocks are copied from the source with their original
 * spacing; edited blocks are serialized one by one and joined with a blank line. Empty
 * paragraphs outside untouched runs are dropped rather than written as `&nbsp;`.
 */
export function serializeDocument(
  doc: Node,
  map: SourceMap,
  manager: MarkdownManager,
): { markdown: string; map: SourceMap } {
  const current: Node[] = [];
  doc.forEach((child) => current.push(child));

  // Nothing touched: the file as it was, byte for byte. Empty paragraphs don't count (they're
  // never written), so the one the editor appends after a trailing list isn't an edit.
  const blocks = current.filter((node) => !isEmptyParagraph(node));
  const originalBlocks = map.nodes.filter((node) => !isEmptyParagraph(node));
  if (blocks.length === originalBlocks.length && blocks.every((node, i) => node === originalBlocks[i])) {
    return { markdown: withEol(map.source, map.eol), map };
  }

  const original = new Map<Node, number>();
  map.nodes.forEach((node, i) => {
    if (map.spans[i]) original.set(node, i);
  });
  // A new node equal to a block from the loaded file (pressing Enter at the end of a
  // paragraph replaces it; undo restores copies) keeps that block's text.
  const sourceOf = (node: Node) =>
    map.loaded.get(textKey(node))?.find((entry) => entry.node.eq(node))?.text ?? null;

  let out = "";
  const spans: (Span | null)[] = current.map(() => null);
  const append = (text: string) => {
    if (out !== "") out += "\n\n";
    const start = out.length;
    out += text;
    return { start, end: out.length };
  };

  for (let i = 0; i < current.length; i += 1) {
    const first = original.get(current[i]);
    if (first === undefined) {
      if (isEmptyParagraph(current[i])) continue;
      const text =
        sourceOf(current[i]) ??
        trimTrailingNewlines(manager.serialize({ type: "doc", content: [current[i].toJSON()] }));
      if (text.trim() !== "") spans[i] = append(text);
      continue;
    }

    // Extend over the following blocks while they are the original's next blocks, unchanged.
    // Stop at a block without verified text: its source is never copied.
    let last = i;
    for (let j = i + 1, k = first + 1; j < current.length && current[j] === map.nodes[k]; j += 1, k += 1) {
      if (map.spans[k]) last = j;
      else if (!isEmptyParagraph(current[j])) break;
    }
    const from = map.spans[first]!.start;
    const runStart = append(map.source.slice(from, map.spans[original.get(current[last])!]!.end)).start;
    for (let j = i; j <= last; j += 1) {
      const span = map.spans[original.get(current[j]) ?? -1];
      if (span) spans[j] = { start: runStart + span.start - from, end: runStart + span.end - from };
    }
    i = last;
  }

  const markdown = out === "" ? "" : `${out}\n`;
  return { markdown: withEol(markdown, map.eol), map: { source: markdown, eol: map.eol, nodes: current, spans, loaded: map.loaded } };
}

const withEol = (text: string, eol: SourceMap["eol"]) => (eol === "\n" ? text : text.replace(/\n/g, eol));
