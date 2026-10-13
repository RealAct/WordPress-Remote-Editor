import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { detectPlatform } from "./lib/platform";
import "./styles/app.css";

void detectPlatform().then((platform) => {
  document.documentElement.dataset.platform = platform;
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <App platform={platform} />
    </StrictMode>,
  );
});

// Native apps don't show a browser context menu on chrome surfaces.
window.addEventListener("contextmenu", (e) => {
  const t = e.target as HTMLElement | null;
  if (!t?.closest("input, textarea, [contenteditable='true']")) e.preventDefault();
});
