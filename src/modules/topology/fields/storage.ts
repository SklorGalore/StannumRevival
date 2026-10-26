import { loadColorSettings, saveColorSettings, defaultColorSettings, type ColorSettings } from "./field-colors";
import type { FilterRule } from "./field-view";

// Filters and colour rules are kept per case file, so reopening restores them.
const FIELD_STORAGE = "stannum-topology-fields:";

export function isComplete(rule: FilterRule): boolean {
  if (rule.operator === "missing" || rule.operator === "present") return true;
  if (rule.value.trim() === "") return false;
  return rule.operator !== "between" || (rule.upper ?? "").trim() !== "";
}

export function loadFieldSettings(path: string | undefined): { rules: FilterRule[]; colors: ColorSettings } {
  try {
    const stored = path ? localStorage.getItem(FIELD_STORAGE + path) : null;
    if (!stored) return { rules: [], colors: defaultColorSettings() };
    const parsed = JSON.parse(stored) as { rules?: unknown; colors?: unknown };
    const rules = Array.isArray(parsed.rules)
      ? (parsed.rules as FilterRule[]).filter((r) => r && typeof r.id === "string" && typeof r.field === "string")
      : [];
    const colors = loadColorSettings({ getItem: () => JSON.stringify(parsed.colors ?? null) });
    return { rules, colors };
  } catch {
    return { rules: [], colors: defaultColorSettings() };
  }
}

export function saveFieldSettings(path: string | undefined, rules: FilterRule[], colors: ColorSettings) {
  if (!path) return;
  let safeColors: unknown = null;
  saveColorSettings(colors, { setItem: (_k: string, v: string) => (safeColors = JSON.parse(v)) });
  try {
    localStorage.setItem(FIELD_STORAGE + path, JSON.stringify({ rules, colors: safeColors }));
  } catch {
    // Storage may be unavailable; the rules still apply for this session.
  }
}
