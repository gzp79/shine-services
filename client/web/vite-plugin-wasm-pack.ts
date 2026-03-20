import { execSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import type { Plugin } from 'vite';

const crateDir = fileURLToPath(new URL('../../crates/shine-game', import.meta.url));
const wasmOut = fileURLToPath(new URL('./pkg/shine_game.js', import.meta.url));

export function wasmPackPlugin(): Plugin {
    return {
        name: 'wasm-pack-watch',
        buildStart() {
            if (!existsSync(wasmOut)) {
                console.log('\n[wasm-pack] pkg/ not found, building...');
                buildWasm();
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
        console.log('[wasm-pack] Done.');
        return true;
    } catch {
        console.error('[wasm-pack] Build failed.');
        return false;
    }
}
