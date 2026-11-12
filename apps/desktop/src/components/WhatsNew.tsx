// The release notes: after an update, and from Settings → About.

import { lang } from "../i18n";
import { useWhatsNew } from "../lib/whatsnew";

const L = (ja: string, en: string) => (lang() === "en" ? en : ja);

/** `code` spans of the notes. */
function Inline({ text }: { text: string }) {
  const parts = text.split(/(`[^`]+`)/);
  return (
    <>
      {parts.map((p, i) => (p.startsWith("`") && p.endsWith("`") ? <code key={i}>{p.slice(1, -1)}</code> : p))}
    </>
  );
}

export function WhatsNewDialog() {
  const { shown, title } = useWhatsNew();
  if (!shown) return null;
  const close = () => useWhatsNew.setState({ shown: null });
  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && close()}>
      <div className="dialog whats-new" role="dialog" aria-modal="true" onKeyDown={(e) => e.key === "Escape" && close()}>
        <div className="dialog-head">{title === "new" ? L("新しくなったこと", "What's new") : L("更新履歴", "Release notes")}</div>
        <div className="dialog-body">
          {shown.map((r) => (
            <section key={r.version}>
              <h4>
                {L("バージョン", "Version")} {r.version}
              </h4>
              <ul>
                {r.items.map((item, i) => (
                  <li key={i}>
                    <Inline text={item} />
                  </li>
                ))}
              </ul>
            </section>
          ))}
        </div>
        <div className="dialog-foot">
          <button className="btn primary" autoFocus onClick={close}>
            {L("閉じる", "Close")}
          </button>
        </div>
      </div>
    </div>
  );
}
