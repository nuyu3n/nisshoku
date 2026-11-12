// A short vibration on phones, for touches that change something (a tick, a
// menu opened by a long press). Nothing elsewhere, nor when it is not allowed.

import { isMobile } from "./mobile";

export function haptic(kind: "light" | "medium" = "light") {
  if (!isMobile) return;
  try {
    navigator.vibrate?.(kind === "light" ? 8 : 16);
  } catch {
    // Not allowed (no permission, or a setting): silent.
  }
}
