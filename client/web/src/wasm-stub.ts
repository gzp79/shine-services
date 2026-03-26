// Stub used when pkg/ hasn't been built yet. Run `pnpm wasm` to build the real module.
import type { WasmPatchMesh } from './wasm-types/shine_game';

export function generate_mesh(_config_json: string): WasmPatchMesh {
    throw new Error('WASM not built. Run `pnpm wasm` first.');
}

export default async function init(): Promise<void> {
    console.warn('Using WASM stub. Run `pnpm wasm` to build the real module.');
}
