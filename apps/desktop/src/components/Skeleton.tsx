// Placeholders in the shape of what is loading, so that the layout is right
// from the first frame and nothing jumps when the content arrives. They fade
// in only after a moment (styles/motion.css), so a quick load shows nothing.

import type { CSSProperties } from "react";

/** One grey bar; `w` is a width (e.g. "60%"), `h` a height in px. */
export function Bone({ w = "100%", h = 12, r, style }: { w?: string | number; h?: number; r?: number; style?: CSSProperties }) {
  return <span className="bone" style={{ width: w, height: h, borderRadius: r, ...style }} />;
}

/** Rows of a note list. */
export function NoteListSkeleton({ rows = 6 }: { rows?: number }) {
  // Varied widths, the same on every render: it looks like text, not a grid.
  const widths = ["62%", "48%", "71%", "55%", "66%", "44%", "58%", "69%"];
  return (
    <div className="skeleton" aria-hidden="true">
      {Array.from({ length: rows }, (_, i) => (
        <div key={i} className="skeleton-note">
          <Bone w={widths[i % widths.length]} h={14} />
          <Bone w="92%" h={10} />
          <Bone w={widths[(i + 3) % widths.length]} h={10} />
          <Bone w={90} h={9} />
        </div>
      ))}
    </div>
  );
}

/** The work panel (logline, progress, actions, tabs, chapters). */
export function WorkPanelSkeleton() {
  return (
    <div className="work-panel skeleton" aria-hidden="true">
      <div className="work-head">
        <Bone w="88%" h={12} />
        <Bone w="64%" h={12} style={{ marginTop: 8 }} />
        <Bone w={120} h={22} style={{ marginTop: 16 }} />
        <Bone w="100%" h={6} r={3} style={{ marginTop: 10 }} />
        <div style={{ display: "flex", gap: 8, marginTop: 14 }}>
          <Bone w={96} h={28} r={8} />
          <Bone w={84} h={28} r={8} />
        </div>
      </div>
      <div className="skeleton-tabs">
        {[0, 1, 2, 3].map((i) => (
          <Bone key={i} w={52} h={14} />
        ))}
      </div>
      <div style={{ padding: "6px 12px" }}>
        {[0, 1, 2, 3, 4].map((i) => (
          <div key={i} className="skeleton-row">
            <Bone w={14} h={12} />
            <Bone w={["70%", "56%", "64%", "48%", "60%"][i]} h={13} />
          </div>
        ))}
      </div>
    </div>
  );
}

/** A plot board: a few columns of cards. */
export function BoardSkeleton() {
  const cards = [3, 2, 4, 1];
  return (
    <div className="plot-board skeleton" aria-hidden="true">
      <div className="plot-head">
        <Bone w={180} h={13} />
      </div>
      <div className="plot-columns">
        {cards.map((n, c) => (
          <div key={c} className="plot-column skeleton-column">
            <Bone w="50%" h={14} style={{ margin: "4px 4px 10px" }} />
            {Array.from({ length: n }, (_, i) => (
              <div key={i} className="skeleton-card">
                <Bone w="80%" h={12} />
                <Bone w="95%" h={10} />
                <Bone w="60%" h={10} />
              </div>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}

/** The bar above a chapter (status, point of view, synopsis, cast). */
export function ChapterBarSkeleton() {
  return (
    <div className="chapter-bar skeleton" aria-hidden="true">
      <div style={{ display: "flex", gap: 8 }}>
        <Bone w={200} h={30} r={8} />
        <Bone w={110} h={30} r={8} />
      </div>
      <Bone w="70%" h={13} style={{ marginTop: 14 }} />
      <div style={{ display: "flex", gap: 8, marginTop: 14 }}>
        <Bone w={70} h={26} r={13} />
        <Bone w={84} h={26} r={13} />
        <Bone w={64} h={26} r={13} />
      </div>
    </div>
  );
}

/** A character sheet. */
export function SheetSkeleton() {
  return (
    <div className="skeleton skeleton-sheet" aria-hidden="true">
      <div style={{ display: "flex", gap: 14, alignItems: "center" }}>
        <Bone w={56} h={56} r={28} />
        <div style={{ flex: 1, display: "grid", gap: 8 }}>
          <Bone w="40%" h={16} />
          <Bone w="25%" h={11} />
        </div>
      </div>
      {[0, 1, 2].map((i) => (
        <div key={i} style={{ display: "grid", gap: 6, marginTop: 16 }}>
          <Bone w={70} h={10} />
          <Bone w="100%" h={30} r={8} />
        </div>
      ))}
    </div>
  );
}

/** The text of a note. */
export function TextSkeleton() {
  return (
    <div className="skeleton skeleton-text" aria-hidden="true">
      {["94%", "88%", "97%", "62%", "", "91%", "85%", "73%"].map((w, i) => (w ? <Bone key={i} w={w} h={14} /> : <span key={i} style={{ height: 14 }} />))}
    </div>
  );
}
