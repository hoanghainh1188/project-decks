import React from "react";
import ReactDOM from "react-dom/client";
import { Root } from "./Root";
import { applyTheme } from "./theme";

// Follow the system until Root reads the saved choice. The backend has already set the
// window to that choice, which the system setting the webview sees follows on macOS.
applyTheme("system");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Root />
  </React.StrictMode>,
);
