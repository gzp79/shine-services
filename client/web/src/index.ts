import init, { generate_mesh } from '#wasm';
import wasmUrl from '#wasm-bin';
import { MeshParams, createControls, defaultParams, paramsToConfigJson } from './controls';
import { HexMeshGroup, buildHexMesh } from './mesh-builder';
import { SceneContext, animate, createScene } from './scene';

export type { MeshParams };

export interface HexMeshViewer {
    destroy(): void;
}

export async function createHexMeshViewer(container: HTMLElement): Promise<HexMeshViewer> {
    await init(wasmUrl);

    const ctx: SceneContext = createScene(container);
    const params = defaultParams();
    let currentMesh: HexMeshGroup | null = null;
    let animationId = 0;

    function regenerate() {
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
            wasmMesh.free();

            currentMesh = buildHexMesh(data);
            ctx.scene.add(currentMesh.group);
        } catch (e) {
            console.error('Mesh generation failed:', e);
        }
    }

    const gui = createControls(container, params, regenerate);
    regenerate();
    animationId = animate(ctx);

    return {
        destroy() {
            cancelAnimationFrame(animationId);
            gui.destroy();
            if (currentMesh) {
                ctx.scene.remove(currentMesh.group);
                currentMesh.dispose();
            }
            ctx.resizeObserver.disconnect();
            ctx.renderer.dispose();
            ctx.renderer.domElement.remove();
        }
    };
}
