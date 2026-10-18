import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Relative base, so the built viewer works under any path of the site (it is published at
// /takt/protocols/ on GitHub Pages).
export default defineConfig({
  base: "./",
  plugins: [vue()],
  build: { outDir: "dist", emptyOutDir: true },
});
