import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import topLevelAwait from 'vite-plugin-top-level-await';
import wasm from 'vite-plugin-wasm';
import { wasmPackPlugin } from './vite-plugin-wasm-pack';

const wasmPath = fileURLToPath(new URL('./pkg/shine_game.js', import.meta.url));
const stubPath = fileURLToPath(new URL('./src/wasm-stub.ts', import.meta.url));

export default defineConfig(({ command }) => ({
    plugins: [wasmPackPlugin(), wasm(), topLevelAwait()],
    resolve: {
        alias: {
            '#wasm': existsSync(wasmPath) ? wasmPath : stubPath
        }
    },
    build:
        command === 'build'
            ? {
                  lib: {
                      entry: fileURLToPath(new URL('./src/index.ts', import.meta.url)),
                      name: 'ShineWeb',
                      fileName: 'shine-web'
                  },
                  rollupOptions: {
                      external: ['three', 'three/addons/controls/OrbitControls.js', 'lil-gui'],
                      output: {
                          globals: {
                              three: 'THREE',
                              'three/addons/controls/OrbitControls.js': 'THREE.OrbitControls',
                              'lil-gui': 'lilGui'
                          }
                      }
                  }
              }
            : {}
}));
