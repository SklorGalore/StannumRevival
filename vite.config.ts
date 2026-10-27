import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { readFileSync } from "node:fs";

// The app version, shown in the rail and written into study files.
const { version } = JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf8")) as { version: string };

export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  define: { __APP_VERSION__: JSON.stringify(version) },
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: {
    target:
      process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
    minify: !process.env.TAURI_ENV_DEBUG,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
    rolldownOptions: {
      // splash.html is the logo window shown while the app loads.
      input: {
        main: "index.html",
        splash: "splash.html",
      },
      output: {
        // Big libraries get chunks of their own so no single file passes
        // Vite's 500 kB warning. Cytoscape only loads with the network view
        // (see LazyNetworkGraph.tsx) and jsPDF only on PDF export.
        codeSplitting: {
          groups: [
            { name: "react", test: /node_modules[\\/](react|react-dom|scheduler)[\\/]/ },
            { name: "xyflow", test: /node_modules[\\/](@xyflow|d3-[^\\/]+|classcat)[\\/]/ },
            { name: "cytoscape", test: /node_modules[\\/]cytoscape[\\/]/ },
            { name: "cytoscape-layouts", test: /node_modules[\\/](cytoscape-fcose|cose-base|layout-base)[\\/]/ },
          ],
        },
      },
    },
  },
}));
