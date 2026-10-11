import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";
import "./surfaces/settings/settings-layout.css";
import "./surfaces/menu-surface-layout.css";
import "./surfaces/settings/electric-edge-settings.css";
import "./surfaces/c1-theme.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
