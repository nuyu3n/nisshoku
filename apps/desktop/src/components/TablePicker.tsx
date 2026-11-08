// The grid for choosing the size of a new table.

import { useEffect, useState } from "react";
import { insertTable, useTablePicker } from "../editor/table";
import { useT } from "../i18n";

const MAX = 10;

export function TablePicker() {
  const t = useT();
  const { at, editor } = useTablePicker();
  const [size, setSize] = useState({ rows: 3, cols: 3 });
  useEffect(() => {
    if (!at) return;
    const close = () => useTablePicker.setState({ at: null });
    const key = (e: KeyboardEvent) => e.key === "Escape" && close();
    window.addEventListener("mousedown", close);
    window.addEventListener("keydown", key);
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("keydown", key);
    };
  }, [at]);
  if (!at || !editor) return null;
  const left = Math.min(at.x, window.innerWidth - 260);
  const top = Math.min(at.y, window.innerHeight - 290);
  return (
    <div className="menu table-picker" style={{ left, top }} onMouseDown={(e) => e.stopPropagation()}>
      <div className="menu-label">
        {t("table.insert")} {size.cols} × {size.rows}
      </div>
      <div className="picker-grid" onMouseLeave={() => setSize({ rows: 3, cols: 3 })}>
        {Array.from({ length: MAX * MAX }, (_, i) => {
          const r = Math.floor(i / MAX) + 1;
          const c = (i % MAX) + 1;
          return (
            <button
              key={i}
              className={r <= size.rows && c <= size.cols ? "on" : ""}
              onMouseEnter={() => setSize({ rows: r, cols: c })}
              onClick={() => {
                useTablePicker.setState({ at: null });
                insertTable(editor, r, c);
              }}
            />
          );
        })}
      </div>
      <div className="picker-hint">{t("table.hint")}</div>
    </div>
  );
}
