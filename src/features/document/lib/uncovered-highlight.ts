import { Extension, type Editor } from "@tiptap/core";
import type { Node } from "@tiptap/pm/model";
import { Plugin, PluginKey, type EditorState } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";
import type { SourceMap } from "./markdown-source";
import { normalizeWhitespace } from "./normalize";

/**
 * Marks the top-level blocks the roadmap didn't pick up, as decorations only: the document
 * itself never changes. A block is marked when it is still the very node whose saved text
 * (whitespace-normalized) equals one of `uncoveredParagraphs`; an edited block is a new
 * node, so it simply stops being marked. Nothing is matched fuzzily.
 */

const key = new PluginKey<DecorationSet>("uncoveredHighlight");

/** The blocks of `map` whose source text is one of `paragraphs`. */
export function uncoveredBlocks(map: SourceMap, paragraphs: readonly string[]): Set<Node> {
  const wanted = new Set(paragraphs.map(normalizeWhitespace));
  const blocks = new Set<Node>();
  if (wanted.size === 0) return blocks;
  map.nodes.forEach((node, i) => {
    const span = map.spans[i];
    if (span && wanted.has(normalizeWhitespace(map.source.slice(span.start, span.end)))) blocks.add(node);
  });
  return blocks;
}

function decorate(state: EditorState, blocks: ReadonlySet<Node>): DecorationSet {
  if (blocks.size === 0) return DecorationSet.empty;
  const decorations: Decoration[] = [];
  state.doc.forEach((node, offset) => {
    if (blocks.has(node)) {
      decorations.push(
        Decoration.node(offset, offset + node.nodeSize, {
          class: "doc-uncovered",
          title: "The roadmap didn't pick this up",
        }),
      );
    }
  });
  return DecorationSet.create(state.doc, decorations);
}

interface HighlightStorage {
  blocks: ReadonlySet<Node>;
}

declare module "@tiptap/core" {
  interface Storage {
    uncoveredHighlight: HighlightStorage;
  }
}

export const UncoveredHighlight = Extension.create<object, HighlightStorage>({
  name: "uncoveredHighlight",

  addStorage() {
    return { blocks: new Set<Node>() };
  },

  addProseMirrorPlugins() {
    const storage = this.storage;
    return [
      new Plugin<DecorationSet>({
        key,
        state: {
          init: (_, state) => decorate(state, storage.blocks),
          // Only a top-level walk with set lookups, and only when the doc or the set changed.
          apply: (tr, previous, _, state) =>
            tr.docChanged || tr.getMeta(key) ? decorate(state, storage.blocks) : previous,
        },
        props: {
          decorations: (state) => key.getState(state),
        },
      }),
    ];
  },
});

/** Replaces the highlighted blocks. Doesn't touch the document or the undo history. */
export function setUncoveredBlocks(editor: Editor, blocks: ReadonlySet<Node>) {
  if (editor.isDestroyed) return;
  editor.storage.uncoveredHighlight.blocks = blocks;
  editor.view.dispatch(editor.state.tr.setMeta(key, true).setMeta("addToHistory", false));
}
