import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import { wasmPackPlugin } from './vite-plugin-wasm-pack';

export default defineConfig(({ command }) => ({
    plugins: [wasmPackPlugin()],
    build:
        command === 'build'
            ? {
                  lib: {
                      entry: fileURLToPath(new URL('./src/index.ts', import.meta.url)),
                      fileName: 'shine-web',
                      formats: ['es']
                  }
              }
            : {}
}));
