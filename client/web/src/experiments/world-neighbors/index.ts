import init, { generate_world_neighbors } from '#wasm';
import wasmUrl from '#wasm-bin';
import * as THREE from 'three';
import { WebGPURenderer } from 'three/webgpu';
import { ExperimentContext, animate, createExperiment } from '../experiment';
import { createControls, defaultParams } from './controls';
import { buildChunkHexagons, buildEdgeMeshes, buildInteriorMeshes, buildVertexMeshes } from './mesh-builder';

export interface WorldNeighborsExperiment {
    dispose(): void;
}

export async function createWorldNeighborsExperiment(
    container: HTMLElement,
    renderer: WebGPURenderer
): Promise<WorldNeighborsExperiment> {
    await init(wasmUrl);

    const ctx: ExperimentContext = createExperiment(container, renderer);
    const params = defaultParams();
    let stopAnimation = () => {};

    let hexagons: THREE.Group | null = null;
    let interiorGroup: ReturnType<typeof buildInteriorMeshes> | null = null;
    let edgeGroup: ReturnType<typeof buildEdgeMeshes> | null = null;
    let vertexGroup: ReturnType<typeof buildVertexMeshes> | null = null;

    function applyDisplay() {
        if (hexagons) hexagons.visible = params.showHexagons;
        if (interiorGroup) {
            for (let i = 0; i < 7; i++) interiorGroup.setIndividualVisible(i, params.showInterior[i]);
        }
        if (edgeGroup) {
            for (let i = 0; i < 6; i++) edgeGroup.setIndividualVisible(i, params.showEdges[i]);
        }
        if (vertexGroup) {
            for (let i = 0; i < 6; i++) vertexGroup.setIndividualVisible(i, params.showVertices[i]);
        }
    }

    function disposeScene() {
        if (hexagons) {
            ctx.scene.remove(hexagons);
            hexagons.traverse((obj) => {
                if (obj instanceof THREE.Line) {
                    obj.geometry.dispose();
                    (obj.material as THREE.Material).dispose();
                }
            });
            hexagons = null;
        }
        if (interiorGroup) {
            ctx.scene.remove(interiorGroup.group);
            interiorGroup.dispose();
            interiorGroup = null;
        }
        if (edgeGroup) {
            ctx.scene.remove(edgeGroup.group);
            edgeGroup.dispose();
            edgeGroup = null;
        }
        if (vertexGroup) {
            ctx.scene.remove(vertexGroup.group);
            vertexGroup.dispose();
            vertexGroup = null;
        }
    }

    function regenerate() {
        disposeScene();
        try {
            const wasmData = generate_world_neighbors(params.centerQ, params.centerR);

            hexagons = buildChunkHexagons(wasmData);
            ctx.scene.add(hexagons);

            interiorGroup = buildInteriorMeshes(wasmData);
            ctx.scene.add(interiorGroup.group);

            edgeGroup = buildEdgeMeshes(wasmData);
            ctx.scene.add(edgeGroup.group);

            vertexGroup = buildVertexMeshes(wasmData);
            ctx.scene.add(vertexGroup.group);

            wasmData.free();
            applyDisplay();
        } catch (e) {
            console.error('World neighbors generation failed:', e);
        }
    }

    ctx.camera.far = 10000;
    ctx.camera.updateProjectionMatrix();
    ctx.camera.position.set(0, -2500, 2000);
    ctx.camera.lookAt(0, 0, 0);
    if (ctx.controls) ctx.controls.update();

    const gui = createControls(container, params, applyDisplay, regenerate);
    regenerate();
    stopAnimation = animate(ctx);

    return {
        dispose() {
            stopAnimation();
            gui.destroy();
            disposeScene();
            ctx.resizeObserver.disconnect();
        }
    };
}
