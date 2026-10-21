import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    proxy: {
      "/api": "http://127.0.0.1:13697",
      "/v1": "http://127.0.0.1:13697",
      "/health": "http://127.0.0.1:13697",
    },
  },
  build: {
    target: "es2022",
    outDir: "dist",
  },
});
