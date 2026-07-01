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
            : {},
    test: {
        environment: 'node',
        include: ['src/**/*.test.ts'],
        resolve: {
            alias: {
                '#wasm': fileURLToPath(new URL('./pkg/shine_game.js', import.meta.url))
            }
        }
    }
}));
