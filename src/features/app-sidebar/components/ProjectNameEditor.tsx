import { LoaderCircle } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { renameProject, type Project } from "@/features/projects";

interface ProjectNameEditorProps {
  project: Project;
  onDone: () => void;
}

/**
 * Inline rename in place of the project row. Enter or blur saves (via updateProject),
 * Escape cancels. A backend error keeps the field open and shows the backend's message.
 */
export function ProjectNameEditor({ project, onDone }: ProjectNameEditorProps) {
  const [value, setValue] = useState(project.name);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const errorId = `rename-${project.id}-error`;

  useEffect(() => {
    // After the menu closes; select all so typing replaces the name.
    const frame = requestAnimationFrame(() => {
      inputRef.current?.focus({ preventScroll: true });
      inputRef.current?.select();
    });
    return () => cancelAnimationFrame(frame);
  }, []);

  const save = async () => {
    if (saving) return;
    if (value.trim() === project.name) return onDone();
    setSaving(true);
    const outcome = await renameProject(project.id, value);
    setSaving(false);
    if (outcome.ok) onDone();
    else {
      setError(outcome.error.message);
      inputRef.current?.focus({ preventScroll: true });
    }
  };

  return (
    <div className="flex flex-col gap-1 pb-1">
      <div className="relative">
        <input
          ref={inputRef}
          value={value}
          onChange={(event) => {
            setValue(event.target.value);
            setError(null);
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              void save();
            } else if (event.key === "Escape") {
              event.preventDefault();
              onDone();
            }
          }}
          onBlur={() => {
            // Leaving a failed edit discards it rather than retrying silently.
            if (error) onDone();
            else void save();
          }}
          readOnly={saving}
          aria-label={`Rename ${project.name}`}
          aria-invalid={error ? true : undefined}
          aria-describedby={error ? errorId : undefined}
          spellCheck={false}
          className="h-8 w-full rounded-xl bg-sidebar-accent px-3 pr-8 text-[12px] text-sidebar-accent-foreground ring-1 ring-sidebar-ring/60 outline-none aria-invalid:ring-destructive/60"
        />
        {saving && (
          <LoaderCircle
            aria-label="Saving"
            className="absolute top-2 right-2.5 size-4 animate-spin text-muted-foreground"
          />
        )}
      </div>
      {error && (
        <p id={errorId} role="alert" className="px-3 text-[11.5px] leading-snug text-destructive">
          {error}
        </p>
      )}
    </div>
  );
}
