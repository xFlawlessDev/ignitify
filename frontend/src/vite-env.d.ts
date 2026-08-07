/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_TEMPLATES_URL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

declare const __IGNITIFY_APP_VERSION__: string;
