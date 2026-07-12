import { useSyncExternalStore } from "react";

/* Theme controller — a single shared store so every consumer (App's wordmark
   variant, Settings' Appearance control, …) stays in sync. Defaults to the OS
   and follows prefers-color-scheme, pinning light/dark only on explicit choice.
   Mirrors the design's app-main.jsx; the tokens stay explicit (data-theme on
   <html>), and a matching pre-paint script in index.html avoids a flash. */

export type ThemeChoice = "system" | "light" | "dark";
export type ResolvedTheme = "light" | "dark";

const STORAGE_KEY = "fotw-theme";

function readStored(): ThemeChoice {
  const v = typeof localStorage !== "undefined" ? localStorage.getItem(STORAGE_KEY) : null;
  return v === "light" || v === "dark" || v === "system" ? v : "system";
}

function systemResolved(): ResolvedTheme {
  return typeof matchMedia !== "undefined" && matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

let choice: ThemeChoice = readStored();
let snapshot: { theme: ThemeChoice; resolved: ResolvedTheme } = {
  theme: choice,
  resolved: choice === "system" ? systemResolved() : choice,
};
const listeners = new Set<() => void>();

/** Recompute the resolved theme, stamp <html>, and refresh the shared snapshot. */
function apply() {
  const resolved: ResolvedTheme = choice === "system" ? systemResolved() : choice;
  if (typeof document !== "undefined") {
    document.documentElement.setAttribute("data-theme", resolved);
  }
  snapshot = { theme: choice, resolved };
}

apply(); // establish data-theme + snapshot at module load

if (typeof matchMedia !== "undefined") {
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
    if (choice === "system") {
      apply();
      listeners.forEach((l) => l());
    }
  });
}

export function setTheme(next: ThemeChoice) {
  choice = next;
  try {
    localStorage.setItem(STORAGE_KEY, next);
  } catch {
    /* storage may be unavailable — theme still applies for the session */
  }
  apply();
  listeners.forEach((l) => l());
}

function subscribe(cb: () => void) {
  listeners.add(cb);
  return () => {
    listeners.delete(cb);
  };
}
function getSnapshot() {
  return snapshot;
}

export function useTheme() {
  const s = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
  return { theme: s.theme, resolved: s.resolved, setTheme };
}
