// Clipboard text: through the app's clipboard plugin (dependable in the
// webview), or the browser's in a browser preview.

import { isTauri } from "../api";

export async function readText(): Promise<string> {
  if (isTauri) {
    const { readText } = await import("@tauri-apps/plugin-clipboard-manager");
    return (await readText()) ?? "";
  }
  return navigator.clipboard.readText();
}

export async function writeText(text: string): Promise<void> {
  if (isTauri) {
    const { writeText } = await import("@tauri-apps/plugin-clipboard-manager");
    return writeText(text);
  }
  return navigator.clipboard.writeText(text);
}
