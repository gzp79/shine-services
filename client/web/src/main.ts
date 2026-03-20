import init, { generate_mesh } from '#wasm';
import { createControls, defaultParams, paramsToConfigJson } from './controls';
import { HexMeshGroup, buildHexMesh } from './mesh-builder';
import { animate, createScene } from './scene';

async function main() {
    await init();

    const ctx = createScene();
    const params = defaultParams();
    let currentMesh: HexMeshGroup | null = null;

    function regenerate() {
        // Remove old mesh
        if (currentMesh) {
            ctx.scene.remove(currentMesh.group);
            currentMesh.dispose();
            currentMesh = null;
        }

        try {
            const configJson = paramsToConfigJson(params);
            const wasmMesh = generate_mesh(configJson);

            const data = {
                vertices: wasmMesh.vertices(),
                indices: wasmMesh.indices(),
                patchIndices: wasmMesh.patch_indices()
            };

            console.log(`Generated: ${wasmMesh.vertex_count()} vertices, ${wasmMesh.quad_count()} quads`);

            // Free wasm-side memory after extracting data
            wasmMesh.free();

            currentMesh = buildHexMesh(data);
            ctx.scene.add(currentMesh.group);
        } catch (e) {
            console.error('Mesh generation failed:', e);
        }
    }

    createControls(params, regenerate);
    regenerate();
    animate(ctx);
}

void main();
