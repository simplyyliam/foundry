import { useCallback, type ClipboardEvent } from "react";

/** A plain file name for a pasted image (the backend refuses anything path-like). */
function pastedName(file: File): string {
  const name = file.name.replace(/[\\/:*?"<>|]+/g, "").trim();
  return name || `pasted.${file.type.split("/")[1] || "png"}`;
}

/**
 * Paste handler for the References view: images on the clipboard become references; text
 * pastes normally (so pasting into a label field still works). Attached to the view itself,
 * not the window, so pasting an image into the brain-dump editor never adds a reference.
 */
export function usePasteImage(onImages: (images: { name: string; bytes: number[] }[]) => void) {
  return useCallback(
    async (event: ClipboardEvent) => {
      const files = Array.from(event.clipboardData.files).filter((file) => file.type.startsWith("image/"));
      if (files.length === 0) return;
      // Copying an image from a browser also puts its URL/HTML on the clipboard; inside a text
      // field, prefer the text. Elsewhere, take the image.
      const target = event.target as HTMLElement;
      const inField = target.closest("input, textarea, [contenteditable=true]") !== null;
      if (inField && event.clipboardData.getData("text/plain")) return;
      event.preventDefault();
      const images = await Promise.all(
        files.map(async (file) => ({ name: pastedName(file), bytes: Array.from(new Uint8Array(await file.arrayBuffer())) })),
      );
      onImages(images);
    },
    [onImages],
  );
}
