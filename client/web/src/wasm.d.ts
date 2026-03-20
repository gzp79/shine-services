declare module '#wasm' {
    export interface WasmPatchMesh {
        vertices(): Float32Array;
        indices(): Uint32Array;
        patch_indices(): Uint8Array;
        vertex_count(): number;
        quad_count(): number;
        free(): void;
    }

    export function generate_mesh(config_json: string): WasmPatchMesh;

    export default function init(): Promise<void>;
}
