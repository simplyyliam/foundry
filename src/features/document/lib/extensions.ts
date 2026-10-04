import { Placeholder } from "@tiptap/extension-placeholder";
import { Markdown } from "@tiptap/markdown";
import { StarterKit } from "@tiptap/starter-kit";
import { UncoveredHighlight } from "./uncovered-highlight";

/** Everything the document editor uses. Markdown shortcuts ("## ", "- ", "**b**") come with StarterKit. */
export function documentExtensions(placeholder: string) {
  return [
    StarterKit.configure({
      // No markdown for underline: Mod-U would add formatting the file can't keep.
      underline: false,
      link: {
        // A click in the webview would navigate the whole app away.
        openOnClick: false,
        autolink: true,
        defaultProtocol: "https",
      },
    }),
    Markdown.configure({ indentation: { style: "space", size: 2 } }),
    Placeholder.configure({ placeholder }),
    UncoveredHighlight,
  ];
}
