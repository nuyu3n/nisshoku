// Opening the sidebar (windows wider than a phone, where it is a drawer):
//
// - Clicking (or tapping) its button opens it: dimmed page, until something
//   is chosen or it is closed. Hovering the button alone does nothing.
// - Dragging a note onto the button shows it over the page — a peek: no
//   dimming — to drop the note on a notebook. The peek stays while the
//   pointer is on the button or the sidebar (or a menu, dialog or the palette
//   opened from it is up), and goes once it has been away for a moment.
// - Pressing anywhere on the peeking sidebar opens it for good.
// - Choosing something in it, Escape, or Ctrl+\ end a peek like an opening.

import { useStore } from "../store";
import { menuOpen } from "../components/ContextMenu";
import { dialogOpen } from "../components/Dialogs";

const CLOSE_AFTER = 350;

let openTimer: number | undefined;
let closeTimer: number | undefined;
let onButton = false;
let onSidebar = false;

const canPeek = () => {
  const s = useStore.getState();
  return s.layout === "medium" && !s.sidebarOpen && !s.focusMode;
};
const hovers = (e: { pointerType: string }) => e.pointerType === "mouse" || e.pointerType === "pen";

function show(after: number) {
  window.clearTimeout(closeTimer);
  window.clearTimeout(openTimer);
  if (!canPeek() || useStore.getState().sidebarPeek) return;
  openTimer = window.setTimeout(() => {
    if (canPeek() && (onButton || onSidebar)) useStore.setState({ sidebarPeek: true });
  }, after);
}

function leaveSoon() {
  window.clearTimeout(openTimer);
  window.clearTimeout(closeTimer);
  closeTimer = window.setTimeout(function check() {
    const s = useStore.getState();
    if (!s.sidebarPeek || onButton || onSidebar) return;
    // Something opened from the sidebar is still up: wait for it.
    if (menuOpen() || dialogOpen() || s.paletteOpen) {
      closeTimer = window.setTimeout(check, 250);
      return;
    }
    useStore.setState({ sidebarPeek: false });
  }, CLOSE_AFTER);
}

/** Pins the sidebar open (a click on its button, or a press on the peeking sidebar). */
export function pinSidebar() {
  window.clearTimeout(openTimer);
  window.clearTimeout(closeTimer);
  useStore.getState().set({ sidebarOpen: true });
}

/** Props of the button that opens the sidebar (hovering alone does not open it). */
export const peekButtonProps = {
  onDragEnter: () => {
    onButton = true;
    show(0);
  },
  onDragLeave: () => {
    onButton = false;
    leaveSoon();
  },
};

const outside = (e: { currentTarget: EventTarget; relatedTarget: EventTarget | null }) => !(e.currentTarget as Node).contains(e.relatedTarget as Node | null);

/** Props of the sidebar itself. */
export const peekSidebarProps = {
  onPointerEnter: (e: React.PointerEvent) => {
    if (!hovers(e)) return;
    onSidebar = true;
    window.clearTimeout(closeTimer);
  },
  // Any press on the peeking sidebar keeps it open, as its button's click does.
  onPointerDown: () => {
    const s = useStore.getState();
    if (s.sidebarPeek && !s.sidebarOpen) pinSidebar();
  },
  onPointerLeave: (e: React.PointerEvent) => {
    if (!hovers(e)) return;
    onSidebar = false;
    leaveSoon();
  },
  onDragEnter: () => {
    onSidebar = true;
    window.clearTimeout(closeTimer);
  },
  onDragLeave: (e: React.DragEvent) => {
    if (!outside(e)) return;
    onSidebar = false;
    leaveSoon();
  },
};

// A drag that ends (dropped on a notebook, or let go) ends its peek.
if (typeof window !== "undefined") {
  const dragEnd = () => {
    if (!useStore.getState().sidebarPeek) return;
    onButton = false;
    onSidebar = false;
    leaveSoon();
  };
  window.addEventListener("dragend", dragEnd);
  window.addEventListener("drop", dragEnd);
  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && useStore.getState().sidebarPeek && !menuOpen() && !dialogOpen()) useStore.setState({ sidebarPeek: false });
  });
}
