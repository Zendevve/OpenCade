import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { strictPort: true },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: "baseline-widely-available",
    minify: !process.env.TAURI_DEBUG ? "oxc" : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
});
