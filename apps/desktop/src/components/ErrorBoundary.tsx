// Catches errors while rendering, so that one broken part (a pane, a dialog)
// does not blank the whole window.

import { Component, type ErrorInfo, type ReactNode } from "react";
import { t } from "../i18n";
import { useStore } from "../store";
import { writeText } from "../lib/clipboard";
import { Icon } from "./Icon";

interface Props {
  children: ReactNode;
  /**
   * - `pane`: shows the error in place of the part, under `className` (so
   *   that the layout keeps its columns and phone screens).
   * - `app`: the same, for the whole window.
   * - `overlay`: closes the part (`onClose`) and reports the error in a toast.
   */
  kind: "pane" | "app" | "overlay";
  className?: string;
  /** Rendering is tried again when this changes (e.g. another note opened). */
  resetKey?: unknown;
  onClose?: () => void;
}

interface State {
  error: Error | null;
  stack: string;
}

function describe(error: Error, stack: string): string {
  return `${error.name}: ${error.message}\n${error.stack ?? ""}\n${stack}`.trim();
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null, stack: "" };

  static getDerivedStateFromError(error: unknown): Partial<State> {
    return { error: error instanceof Error ? error : new Error(String(error)) };
  }

  componentDidCatch(error: unknown, info: ErrorInfo) {
    const stack = info.componentStack ?? "";
    console.error("render error", error, stack);
    // The whole app failed: its message must not stay under the startup screen.
    if (this.props.kind === "app") document.getElementById("splash")?.remove();
    this.setState({ stack });
    if (this.props.kind === "overlay" && this.props.onClose) {
      this.props.onClose();
      // The toasts live in the store, outside this part.
      try {
        useStore.getState().toast(t("error.closed"));
      } catch {
        // The store itself is broken: nothing more to do.
      }
      this.setState({ error: null, stack: "" });
    }
  }

  componentDidUpdate(prev: Props) {
    if (this.state.error && prev.resetKey !== this.props.resetKey) this.setState({ error: null, stack: "" });
  }

  render() {
    const { error, stack } = this.state;
    if (!error) return this.props.children;
    if (this.props.kind === "overlay") return null;
    const app = this.props.kind === "app";
    return (
      <div className={`${this.props.className ?? ""} error-pane${app ? " error-app" : ""}`} role="alert">
        <div className="error-box">
          <Icon name="alert" />
          <b>{t("error.title")}</b>
          <p>{t("error.message")}</p>
          <code>{error.message}</code>
          <div className="row-buttons">
            {!app && (
              <button className="btn primary" onClick={() => this.setState({ error: null, stack: "" })}>
                {t("error.retry")}
              </button>
            )}
            <button className={`btn${app ? " primary" : ""}`} onClick={() => window.location.reload()}>
              {t("error.reload")}
            </button>
            <button
              className="btn"
              onClick={() =>
                void writeText(describe(error, stack)).then(
                  () => !app && useStore.getState().toast(t("error.copied")),
                  () => {},
                )
              }
            >
              {t("error.copy")}
            </button>
          </div>
        </div>
      </div>
    );
  }
}
