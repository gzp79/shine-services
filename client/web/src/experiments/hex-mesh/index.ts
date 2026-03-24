import init, { generate_mesh } from '#wasm';
import wasmUrl from '#wasm-bin';
import { MeshParams, createControls, defaultGlobalParams, defaultParams, paramsToConfigJson } from './controls';
import { HexMeshGroup, buildHexMesh } from './mesh-builder';
import { SceneContext, animate, createScene } from '../../scene';

export type { MeshParams };

export interface HexMeshViewer {
    destroy(): void;
}

export async function createHexMeshViewer(container: HTMLElement): Promise<HexMeshViewer> {
    await init(wasmUrl);

    const ctx: SceneContext = createScene(container);
    const params = defaultParams();
    const globalParams = defaultGlobalParams();
    let currentMesh: HexMeshGroup | null = null;
    let animationId = 0;

    function applyDisplay() {
        if (currentMesh) {
            currentMesh.setPrimalVisible(globalParams.showPrimal);
            currentMesh.setDualVisible(globalParams.showDual);
        }
    }

    function regenerate() {
        if (currentMesh) {
            ctx.scene.remove(currentMesh.group);
            currentMesh.dispose();
            currentMesh = null;
        }

        try {
            const configJson = paramsToConfigJson(params, globalParams);
            const wasmMesh = generate_mesh(configJson);

            const data = {
                vertices: wasmMesh.vertices(),
                indices: wasmMesh.indices(),
                patchIndices: wasmMesh.patch_indices(),
                dualVertices: wasmMesh.dual_vertices(),
                dualIndices: wasmMesh.dual_indices()
            };

            console.log(
                `Generated: ${wasmMesh.vertex_count()} vertices, ${wasmMesh.quad_count()} quads, ${wasmMesh.dual_edge_count()} dual edges`
            );
            wasmMesh.free();

            currentMesh = buildHexMesh(data);
            applyDisplay();
            ctx.scene.add(currentMesh.group);
        } catch (e) {
            console.error('Mesh generation failed:', e);
        }
    }

    const gui = createControls(container, params, globalParams, regenerate, applyDisplay);
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
