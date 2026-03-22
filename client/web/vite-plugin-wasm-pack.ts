import { execSync } from 'node:child_process';
import { copyFileSync, existsSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import type { Plugin } from 'vite';

const crateDir = fileURLToPath(new URL('../../crates/shine-game', import.meta.url));
const wasmOut = fileURLToPath(new URL('./pkg/shine_game.js', import.meta.url));
const wasmBin = fileURLToPath(new URL('./pkg/shine_game_bg.wasm', import.meta.url));
const wasmTypes = fileURLToPath(new URL('./pkg/shine_game.d.ts', import.meta.url));
const typesDir = fileURLToPath(new URL('./src/wasm-types/shine_game.d.ts', import.meta.url));

const WASM_BIN_ID = '#wasm-bin';
const WASM_BIN_RESOLVED = '\0wasm-bin';

export function wasmPackPlugin(): Plugin {
    let isBuild = false;

    return {
        name: 'wasm-pack',
        enforce: 'pre',
        config(_, env) {
            isBuild = env.command === 'build';
            if (!existsSync(wasmOut)) {
                console.log('\n[wasm-pack] pkg/ not found, building...');
                buildWasm();
            }
            return {
                resolve: {
                    alias: {
                        '#wasm': wasmOut
                    }
                }
            };
        },
        resolveId(id) {
            if (id === WASM_BIN_ID) {
                return WASM_BIN_RESOLVED;
            }
        },
        load(id) {
            if (id === WASM_BIN_RESOLVED) {
                if (isBuild) {
                    const wasmSource = readFileSync(wasmBin);
                    const refId = this.emitFile({
                        type: 'asset',
                        name: 'shine_game_bg.wasm',
                        source: wasmSource
                    });
                    return `export default import.meta.ROLLUP_FILE_URL_${refId};`;
                }
                // Dev mode: serve wasm relative to project root
                return 'export default "/pkg/shine_game_bg.wasm";';
            }
        },
        configureServer(server) {
            const srcDir = `${crateDir}/src`;
            server.watcher.add(srcDir);
            server.watcher.on('change', (path) => {
                if (path.endsWith('.rs')) {
                    console.log(`\n[wasm-pack] Rust file changed: ${path}`);
                    if (buildWasm()) {
                        void server.restart();
                    }
                }
            });
        }
    };
}

function buildWasm(): boolean {
    try {
        console.log('[wasm-pack] Building...');
        execSync('wasm-pack build --target web --out-dir ../../client/web/pkg', {
            cwd: crateDir,
            stdio: 'inherit'
        });
        copyFileSync(wasmTypes, typesDir);
        console.log('[wasm-pack] Done.');
        return true;
    } catch {
        console.error('[wasm-pack] Build failed.');
        return false;
    }
}
