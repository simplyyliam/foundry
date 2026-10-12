import type { RequirementCheck } from "@/lib/bindings";

/**
 * Requirement checks in plain words, shared by the Requirements page (editing them) and the
 * Verification sheet (their results), which can't import each other.
 */

export type CheckType = RequirementCheck["type"];
/** The text fields a check type takes, in the order the add form shows them. */
export type CheckField = "path" | "name" | "glob" | "pattern";

export const CHECK_TYPES: { type: CheckType; label: string; fields: CheckField[] }[] = [
  { type: "file_exists", label: "File must exist", fields: ["path"] },
  { type: "file_absent", label: "File must not exist", fields: ["path"] },
  { type: "dependency_present", label: "Dependency must be present", fields: ["name"] },
  { type: "dependency_absent", label: "Dependency must not be present", fields: ["name"] },
  { type: "text_absent", label: "Text must not appear", fields: ["glob", "pattern"] },
];

/** One sentence, e.g. "File components.json must exist" or "No match for <pattern> in <glob>". */
export function describeCheck(check: RequirementCheck): string {
  switch (check.type) {
    case "file_exists":
      return `File ${check.path} must exist`;
    case "file_absent":
      return `File ${check.path} must not exist`;
    case "dependency_present":
      return `Dependency ${check.name} must be present`;
    case "dependency_absent":
      return `Dependency ${check.name} must not be present`;
    case "text_absent":
      return `No match for ${check.pattern} in ${check.glob}`;
  }
}

/**
 * The check the add form describes, or null while a field it needs is blank. Values are
 * trimmed; the backend validates the rest (paths inside the project, pattern size).
 */
export function buildCheck(type: CheckType, values: Partial<Record<CheckField, string>>): RequirementCheck | null {
  const value = (field: CheckField) => values[field]?.trim() ?? "";
  switch (type) {
    case "file_exists":
    case "file_absent":
      return value("path") ? { type, path: value("path") } : null;
    case "dependency_present":
    case "dependency_absent":
      return value("name") ? { type, name: value("name") } : null;
    case "text_absent":
      return value("glob") && value("pattern") ? { type, glob: value("glob"), pattern: value("pattern") } : null;
  }
}

/** Same check: same type and fields (used to hide dismissed suggestions and skip duplicates). */
export function checkKey(check: RequirementCheck): string {
  switch (check.type) {
    case "file_exists":
    case "file_absent":
      return `${check.type}\u0000${check.path}`;
    case "dependency_present":
    case "dependency_absent":
      return `${check.type}\u0000${check.name}`;
    case "text_absent":
      return `${check.type}\u0000${check.glob}\u0000${check.pattern}`;
  }
}
