import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    // cargo writes build tools into target/ while Vite runs; watching them crashes Vite on Windows (EBUSY)
    watch: { ignored: ["**/target/**", "**/src-tauri/**", "**/core/**", "**/client-mod/**", "**/node_modules/**"] }
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: { target: "es2021", sourcemap: false }
});
