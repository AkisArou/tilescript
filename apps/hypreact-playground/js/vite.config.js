import { defineConfig } from "vite";
import { resolve } from "node:path";

export default defineConfig({
  base: "./",
  build: {
    lib: {
      entry: {
        "monaco-host": resolve(import.meta.dirname, "src/monaco-host.ts"),
        "lua-runtime": resolve(import.meta.dirname, "src/lua-runtime.ts"),
      },
      formats: ["es"],
      fileName: (_format, entryName) => `playground-assets/${entryName}.js`,
    },
    outDir: resolve(import.meta.dirname, "dist"),
    emptyOutDir: false,
  },
});
