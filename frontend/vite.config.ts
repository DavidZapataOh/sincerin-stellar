import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    strictPort: false,
  },
  build: {
    // The wallet kit (+ its crypto/WalletConnect deps) is the bulk of the bundle
    // and is already lazy-imported on connect; this split keeps it out of the
    // initial chunk and silences the size warning for an expected-large vendor.
    chunkSizeWarningLimit: 700,
  },
});
