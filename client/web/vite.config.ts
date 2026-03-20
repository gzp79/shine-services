import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import topLevelAwait from 'vite-plugin-top-level-await';
import wasm from 'vite-plugin-wasm';

export default defineConfig({
    plugins: [wasm(), topLevelAwait()],
    resolve: {
        alias: {
            '#wasm': fileURLToPath(new URL('./pkg/shine_game.js', import.meta.url))
        }
    }
});
