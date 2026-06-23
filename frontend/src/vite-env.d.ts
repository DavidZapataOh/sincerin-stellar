/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_SEQUENCER_URL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
