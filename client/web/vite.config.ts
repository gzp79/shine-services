import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import topLevelAwait from 'vite-plugin-top-level-await';
import wasm from 'vite-plugin-wasm';
import { wasmPackPlugin } from './vite-plugin-wasm-pack';

export default defineConfig(({ command }) => ({
    plugins: [wasmPackPlugin(), wasm(), topLevelAwait()],
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
