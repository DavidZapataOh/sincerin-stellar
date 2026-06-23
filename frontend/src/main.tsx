import { Buffer } from "buffer";
// @stellar/stellar-base (StrKey decode) + the wallets kit expect a Node `Buffer`
// global in the browser. Polyfill it before any Stellar import is evaluated.
if (typeof globalThis.Buffer === "undefined") {
  globalThis.Buffer = Buffer;
}

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/app.css";

const root = document.getElementById("root");
if (!root) throw new Error("#root not found");

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
