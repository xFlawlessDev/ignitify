import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite-plus";
import tailwindcss from "@tailwindcss/vite";
import vue from "@vitejs/plugin-vue";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [vue(), tailwindcss()],
  resolve: {
    alias: { "@": path.resolve(__dirname, "./src") },
  },
  server: {
    port: 6565,
    host: true,
    allowedHosts: ["localhost", "127.0.0.1"],
    proxy: {
      "/api": {
        target: "http://127.0.0.1:5656",
        changeOrigin: true,
        ws: true,
      },
    },
  },
  build: { outDir: "dist" },
  staged: {
    "*": "vp check --fix",
  },
  fmt: { ignorePatterns: ["**/*.md"] },
  lint: { options: { typeAware: true, typeCheck: true } },
  test: {
    include: ["src/**/*.{test,spec}.{js,ts,jsx,tsx}"],
    testTimeout: 10_000,
  },
  run: { cache: true },
});
