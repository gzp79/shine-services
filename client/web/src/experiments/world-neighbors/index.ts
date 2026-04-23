import init, { generate_world_neighbors } from '#wasm';
import wasmUrl from '#wasm-bin';
import * as THREE from 'three';
import { ExperimentContext, animate, createExperiment } from '../experiment';
import { createControls, defaultParams } from './controls';
import { buildChunkHexagons, buildEdgeMeshes, buildInteriorMeshes, buildVertexMeshes } from './mesh-builder';

export interface WorldNeighborsExperiment {
    dispose(): void;
}

export async function createWorldNeighborsExperiment(container: HTMLElement): Promise<WorldNeighborsExperiment> {
    await init(wasmUrl);

    const ctx: ExperimentContext = await createExperiment(container);
    const params = defaultParams();
    let animationId = 0;

    try {
        const wasmData = generate_world_neighbors();

        // Build chunk hexagons (always visible)
        const hexagons = buildChunkHexagons(wasmData);
        ctx.scene.add(hexagons);

        // Build interior meshes (toggleable)
        const interiorGroup = buildInteriorMeshes(wasmData);
        ctx.scene.add(interiorGroup.group);

        // Build edge meshes (toggleable)
        const edgeGroup = buildEdgeMeshes(wasmData);
        ctx.scene.add(edgeGroup.group);

        // Build vertex meshes (toggleable)
        const vertexGroup = buildVertexMeshes(wasmData);
        ctx.scene.add(vertexGroup.group);

        wasmData.free();

        function applyDisplay() {
            hexagons.visible = params.showHexagons;
            for (let i = 0; i < 7; i++) {
                interiorGroup.setIndividualVisible(i, params.showInterior[i]);
            }
            for (let i = 0; i < 6; i++) {
                edgeGroup.setIndividualVisible(i, params.showEdges[i]);
                vertexGroup.setIndividualVisible(i, params.showVertices[i]);
            }
        }

        const gui = createControls(container, params, applyDisplay);
        applyDisplay();

        // Position camera to view all 7 chunks (CHUNK_WORLD_SIZE = 1000)
        ctx.camera.far = 10000;
        ctx.camera.updateProjectionMatrix();
        ctx.camera.position.set(0, -2500, 2000);
        ctx.camera.lookAt(0, 0, 0);
        if (ctx.controls) {
            ctx.controls.update();
        }

        animationId = animate(ctx);

        return {
            dispose() {
                cancelAnimationFrame(animationId);
                gui.destroy();
                ctx.scene.remove(hexagons);
                ctx.scene.remove(interiorGroup.group);
                ctx.scene.remove(edgeGroup.group);
                ctx.scene.remove(vertexGroup.group);
                interiorGroup.dispose();
                edgeGroup.dispose();
                vertexGroup.dispose();
                hexagons.traverse((obj) => {
                    if (obj instanceof THREE.Line) {
                        obj.geometry.dispose();
                        (obj.material as THREE.Material).dispose();
                    }
                });
                ctx.resizeObserver.disconnect();
                ctx.renderer.dispose();
                ctx.renderer.domElement.remove();
            }
        };
    } catch (e) {
        console.error('World neighbors generation failed:', e);
        throw e;
    }
}
