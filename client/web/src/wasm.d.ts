declare module '#wasm' {
    import type { WasmPatchMesh } from './wasm-types';

    export { WasmPatchMesh };
    export function generate_mesh(config_json: string): WasmPatchMesh;
    export default function init(): Promise<void>;
}
