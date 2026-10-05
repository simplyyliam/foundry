import { useState } from "react";
import { Plus } from "lucide-react";
import { Input } from "@/components/ui/input";
import { PressButton } from "@/shared";

interface InlineAddProps {
  placeholder: string;
  label: string;
  busy?: boolean;
  /** Resolves true when it worked; the field then clears. */
  onSubmit: (text: string) => Promise<boolean>;
}

/** A one-line field that adds an item on Enter (or the + button). */
export function InlineAdd({ placeholder, label, busy, onSubmit }: InlineAddProps) {
  const [text, setText] = useState("");
  const empty = text.trim() === "";

  return (
    <form
      className="flex items-center gap-1.5"
      onSubmit={async (event) => {
        event.preventDefault();
        if (empty || busy) return;
        if (await onSubmit(text.trim())) setText("");
      }}
    >
      <Input
        value={text}
        onChange={(event) => setText(event.target.value)}
        placeholder={placeholder}
        aria-label={label}
        className="h-8 text-[13px]"
      />
      <PressButton pressScale={0.92} type="submit" variant="secondary" size="icon-sm" aria-label={label} disabled={empty || busy}>
        <Plus aria-hidden />
      </PressButton>
    </form>
  );
}
