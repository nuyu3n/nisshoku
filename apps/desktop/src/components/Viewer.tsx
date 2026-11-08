// A viewer for PDF attachments, inside the app.

import { api } from "../api";
import { report, useStore } from "../store";
import { useT } from "../i18n";
import { Icon } from "./Icon";

export function AttachmentViewer() {
  const t = useT();
  const preview = useStore((s) => s.preview);
  if (!preview) return null;
  const close = () => useStore.getState().set({ preview: null });
  return (
    <div className="overlay viewer-overlay" onMouseDown={(e) => e.target === e.currentTarget && close()} onKeyDown={(e) => e.key === "Escape" && close()}>
      <div className="dialog viewer">
        <div className="side-panel-head">
          <span className="grow">{preview.title}</span>
          <button className="btn" onClick={() => void api.openResource(preview.id).catch(report)}>
            {t("viewer.open")}
          </button>
          <button className="icon-btn" title={t("panel.close")} onClick={close}>
            <Icon name="x" />
          </button>
        </div>
        <iframe title={preview.title} src={preview.url} />
      </div>
    </div>
  );
}
