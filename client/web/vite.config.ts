import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import { wasmPackPlugin } from './vite-plugin-wasm-pack';

export default defineConfig(({ command }) => {
    // Heap profiling (counting allocator + heap_* exports) is opt-in dev tooling. Defaults on for
    // dev serve, off for the production build; override with HEAP_PROFILING=true|false. This single
    // flag both selects the wasm feature and is exposed to TS as import.meta.env.VITE_HEAP_PROFILING.
    const heapProfiling = process.env.HEAP_PROFILING ? process.env.HEAP_PROFILING === 'true' : command === 'serve';

    return {
        plugins: [wasmPackPlugin({ profiling: heapProfiling })],
        define: {
            'import.meta.env.VITE_HEAP_PROFILING': JSON.stringify(heapProfiling)
        },
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
    };
});
