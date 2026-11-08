import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { isTauri } from "./api";
import { hideSplash } from "./lib/motion";
import "./styles/tokens.css";
import "./styles/app.css";
import "./styles/parts.css";
import "./styles/layout.css";
import "./styles/features.css";
import "./styles/works.css";
import "./styles/genko.css";
import "./styles/controls.css";
import "./styles/motion.css";
import "katex/dist/katex.min.css";

const root = createRoot(document.getElementById("root")!);
if (!isTauri && new URLSearchParams(window.location.search).has("selftest")) {
  hideSplash();
  void import("./dev/selftest").then(({ SelfTest }) => root.render(<SelfTest />));
} else {
  root.render(
    <StrictMode>
      <ErrorBoundary kind="app" className="app">
        <App />
      </ErrorBoundary>
    </StrictMode>,
  );
}

// Whatever happens at startup, the startup screen never stays for good.
window.setTimeout(hideSplash, 8000);
