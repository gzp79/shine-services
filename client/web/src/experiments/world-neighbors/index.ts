import init, { generate_world_neighbors } from '#wasm';
import wasmUrl from '#wasm-bin';
import * as THREE from 'three';
import { WebGPURenderer } from 'three/webgpu';
import { Experiment } from '../experiment';
import { createControls, defaultParams } from './controls';
import { buildChunkHexagons, buildEdgeMeshes, buildInteriorMeshes, buildVertexMeshes } from './mesh-builder';

export interface WorldNeighborsExperiment {
    dispose(): void;
}

class WorldNeighbors extends Experiment {
    private params = defaultParams();
    private hexagons: THREE.Group | null = null;
    private interiorGroup: ReturnType<typeof buildInteriorMeshes> | null = null;
    private edgeGroup: ReturnType<typeof buildEdgeMeshes> | null = null;
    private vertexGroup: ReturnType<typeof buildVertexMeshes> | null = null;
    private gui: import('lil-gui').GUI;

    constructor(container: HTMLElement, renderer: WebGPURenderer) {
        super(container, renderer);

        this.camera.far = 10000;
        this.camera.updateProjectionMatrix();
        this.camera.position.set(0, -2500, 2000);
        this.camera.lookAt(0, 0, 0);
        if (this.controls) this.controls.update();

        this.gui = createControls(container, this.params, () => this.applyDisplay(), () => this.regenerate());
        this.regenerate();
        this.start();
    }

    private applyDisplay() {
        if (this.hexagons) this.hexagons.visible = this.params.showHexagons;
        if (this.interiorGroup) {
            for (let i = 0; i < 7; i++) this.interiorGroup.setIndividualVisible(i, this.params.showInterior[i]);
        }
        if (this.edgeGroup) {
            for (let i = 0; i < 6; i++) this.edgeGroup.setIndividualVisible(i, this.params.showEdges[i]);
        }
        if (this.vertexGroup) {
            for (let i = 0; i < 6; i++) this.vertexGroup.setIndividualVisible(i, this.params.showVertices[i]);
        }
    }

    private disposeScene() {
        if (this.hexagons) {
            this.scene.remove(this.hexagons);
            this.hexagons.traverse((obj) => {
                if (obj instanceof THREE.Line) {
                    obj.geometry.dispose();
                    (obj.material as THREE.Material).dispose();
                }
            });
            this.hexagons = null;
        }
        if (this.interiorGroup) {
            this.scene.remove(this.interiorGroup.group);
            this.interiorGroup.dispose();
            this.interiorGroup = null;
        }
        if (this.edgeGroup) {
            this.scene.remove(this.edgeGroup.group);
            this.edgeGroup.dispose();
            this.edgeGroup = null;
        }
        if (this.vertexGroup) {
            this.scene.remove(this.vertexGroup.group);
            this.vertexGroup.dispose();
            this.vertexGroup = null;
        }
    }

    private regenerate() {
        this.disposeScene();
        try {
            const wasmData = generate_world_neighbors(this.params.centerQ, this.params.centerR);

            this.hexagons = buildChunkHexagons(wasmData);
            this.scene.add(this.hexagons);

            this.interiorGroup = buildInteriorMeshes(wasmData);
            this.scene.add(this.interiorGroup.group);

            this.edgeGroup = buildEdgeMeshes(wasmData);
            this.scene.add(this.edgeGroup.group);

            this.vertexGroup = buildVertexMeshes(wasmData);
            this.scene.add(this.vertexGroup.group);

            wasmData.free();
            this.applyDisplay();
        } catch (e) {
            console.error('World neighbors generation failed:', e);
        }
    }

    dispose() {
        this.gui.destroy();
        this.disposeScene();
        super.dispose();
    }
}

export async function createWorldNeighborsExperiment(
    container: HTMLElement,
    renderer: WebGPURenderer
): Promise<WorldNeighborsExperiment> {
    await init(wasmUrl);
    return new WorldNeighbors(container, renderer);
}
