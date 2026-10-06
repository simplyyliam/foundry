import { Check, LoaderCircle, Search } from "lucide-react";
import { useState, type KeyboardEvent } from "react";
import { Input } from "@/components/ui/input";
import type { HandbookRole } from "@/lib/bindings";
import { cn } from "@/lib/utils";
import { PressButton, useSettingsDialog } from "@/shared";
import type { HandbookRoles } from "../hooks/useHandbookRoles";
import { ErrorLine } from "./ErrorLine";

interface HandbookRoleListProps {
  handbook: HandbookRoles;
  onPick: (role: HandbookRole) => void;
  /** Marks the picked role (the Add agent dialog); the popover adds on click instead. */
  selectedId?: string | null;
  autoFocus?: boolean;
}

/**
 * The Role Handbook as a filterable list. Enter in the filter picks the first match; with no
 * roles, it links to Settings → Role Handbook.
 */
export function HandbookRoleList({ handbook, onPick, selectedId, autoFocus }: HandbookRoleListProps) {
  const { openSettings } = useSettingsDialog();
  const [query, setQuery] = useState("");
  const { roles, error } = handbook;

  if (error && !roles) return <ErrorLine message={error} />;
  if (!roles) {
    return (
      <div className="flex justify-center py-6">
        <LoaderCircle aria-label="Loading the Role Handbook" className="size-4 animate-spin text-muted-foreground" />
      </div>
    );
  }
  if (roles.length === 0) {
    return (
      <p className="py-2 text-[12.5px] leading-snug text-muted-foreground">
        The Role Handbook is empty.{" "}
        <PressButton
          type="button"
          variant="link"
          className="h-auto p-0 text-[12.5px] text-foreground underline underline-offset-2"
          onClick={() => openSettings("role-handbook")}
        >
          Add roles in Settings
        </PressButton>
      </p>
    );
  }

  const needle = query.trim().toLocaleLowerCase();
  const matches = needle
    ? roles.filter((role) => `${role.title}\n${role.description}`.toLocaleLowerCase().includes(needle))
    : roles;

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter" && matches[0]) {
      event.preventDefault();
      onPick(matches[0]);
    }
  };

  return (
    <div className="flex min-h-0 flex-col gap-2">
      {error && <ErrorLine message={error} />}
      <div className="relative">
        <Search aria-hidden className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground" />
        <Input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={onKeyDown}
          placeholder="Filter roles"
          aria-label="Filter roles"
          autoFocus={autoFocus}
          className="h-8 pl-8 text-[13px]"
        />
      </div>
      <ul role="list" aria-label="Role Handbook" className="-mx-1 flex max-h-64 flex-col gap-0.5 overflow-y-auto px-1">
        {matches.map((role) => {
          const selected = role.id === selectedId;
          return (
            <li key={role.id}>
              <PressButton
                type="button"
                variant="ghost"
                pressScale={0.98}
                aria-pressed={selectedId === undefined ? undefined : selected}
                onClick={() => onPick(role)}
                className={cn(
                  "h-auto w-full items-start justify-start gap-2 rounded-md px-2 py-1.5 text-left whitespace-normal",
                  selected && "bg-muted",
                )}
              >
                <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                  <span className="text-[13px] font-medium">{role.title}</span>
                  {role.description && (
                    <span className="line-clamp-2 text-[12px] leading-snug font-normal text-muted-foreground">{role.description}</span>
                  )}
                </span>
                {selected && <Check aria-hidden className="mt-0.5 size-3.5 shrink-0 text-primary" />}
              </PressButton>
            </li>
          );
        })}
        {matches.length === 0 && <li className="px-2 py-3 text-[12.5px] text-muted-foreground">No role matches “{query.trim()}”.</li>}
      </ul>
    </div>
  );
}
