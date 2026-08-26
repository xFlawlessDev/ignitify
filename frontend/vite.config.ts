import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite-plus";
import tailwindcss from "@tailwindcss/vite";
import vue from "@vitejs/plugin-vue";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const packageManifest = JSON.parse(
  readFileSync(new URL("./package.json", import.meta.url), "utf8"),
) as { version: string };
const appVersion = process.env.IGNITIFY_APP_VERSION?.trim() || packageManifest.version;

export default defineConfig({
  define: {
    __IGNITIFY_APP_VERSION__: JSON.stringify(appVersion),
  },
  plugins: [vue(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
      // Compile Sonner from source so its SFC styles are emitted in the app CSS
      // instead of injected into the document at runtime.
      "vue-sonner": path.resolve(__dirname, "./node_modules/vue-sonner/src/packages/index.ts"),
    },
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
    setupFiles: ["src/test/setup.ts"],
    testTimeout: 10_000,
    fileParallelism: false,
  },
  run: { cache: true },
});
