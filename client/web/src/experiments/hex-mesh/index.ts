import init, { generate_mesh } from '#wasm';
import wasmUrl from '#wasm-bin';
import * as THREE from 'three';
import { ExperimentContext, animate, createExperiment } from '../experiment';
import { createControls, defaultParams, paramsToConfigJson } from './controls';
import { HexMeshGroup, buildHexMesh } from './mesh-builder';

export interface HexMeshExperiment {
    dispose(): void;
}

export async function createHexMeshExperiment(container: HTMLElement): Promise<HexMeshExperiment> {
    await init(wasmUrl);

    const ctx: ExperimentContext = await createExperiment(container);
    const params = defaultParams();
    let currentMesh: HexMeshGroup | null = null;
    let debugCircle: THREE.Mesh | null = null;
    let axesGroup: THREE.Group | null = null;
    let animationId = 0;

    function applyDisplay() {
        if (currentMesh) {
            currentMesh.setPrimalMeshVisible(params.showPrimalMesh);
            currentMesh.setPrimalWireVisible(params.showPrimalWire);
            currentMesh.setDualMeshVisible(params.showDualMesh);
            currentMesh.setDualWireVisible(params.showDualWire);
            currentMesh.setAnchorVisible(params.showAnchor);
            currentMesh.setAnchorVerticesVisible(params.showAnchorVertices);
        }
    }

    function regenerate() {
        if (currentMesh) {
            ctx.scene.remove(currentMesh.group);
            currentMesh.dispose();
            currentMesh = null;
        }
        if (debugCircle) {
            ctx.scene.remove(debugCircle);
            debugCircle.geometry.dispose();
            (debugCircle.material as THREE.Material).dispose();
            debugCircle = null;
        }
        if (axesGroup) {
            ctx.scene.remove(axesGroup);
            axesGroup.traverse((obj) => {
                if (obj instanceof THREE.Line) {
                    obj.geometry.dispose();
                    (obj.material as THREE.Material).dispose();
                }
            });
            axesGroup = null;
        }

        try {
            const configJson = paramsToConfigJson(params);
            const wasmMesh = generate_mesh(configJson);

            const worldSize = wasmMesh.world_size();
            const primal = wasmMesh.primal();
            const dual = wasmMesh.dual();

            const data = {
                vertices: primal.vertices(),
                quad_indices: primal.indices(),
                quad_ranges: primal.polygon_ranges(),
                anchor_indices: primal.wire_indices(),
                anchor_ranges: primal.wire_ranges(),
                dual_vertices: dual.vertices(),
                dual_indices: dual.indices(),
                dual_ranges: dual.polygon_ranges()
            };

            primal.free();
            dual.free();
            wasmMesh.free();

            currentMesh = buildHexMesh(data);
            applyDisplay();
            ctx.scene.add(currentMesh.group);

            // Debug circle showing world size
            const ringGeom = new THREE.RingGeometry(0, worldSize, 64);
            const ringMat = new THREE.MeshBasicMaterial({ color: 0x6f6f6f, side: THREE.DoubleSide });
            debugCircle = new THREE.Mesh(ringGeom, ringMat);
            debugCircle.position.z = -0.001;
            ctx.scene.add(debugCircle);

            // Coordinate axes: X=red, Y=green, Z=blue, length = worldSize/6
            const axisLen = worldSize / 6;
            axesGroup = new THREE.Group();
            const axes: [THREE.Vector3, number][] = [
                [new THREE.Vector3(axisLen, 0, 0), 0xff0000], // X = red
                [new THREE.Vector3(0, axisLen, 0), 0x00ff00], // Y = green
                [new THREE.Vector3(0, 0, axisLen), 0x0000ff] // Z = blue
            ];
            const origin = new THREE.Vector3(0, 0, 0);
            for (const [dir, color] of axes) {
                const geom = new THREE.BufferGeometry().setFromPoints([origin, dir]);
                const mat = new THREE.LineBasicMaterial({ color });
                axesGroup.add(new THREE.Line(geom, mat));
            }
            axesGroup.position.z = 0.02;
            ctx.scene.add(axesGroup);
        } catch (e) {
            console.error('Mesh generation failed:', e);
        }
    }

    const gui = createControls(container, params, regenerate, applyDisplay);
    regenerate();
    animationId = animate(ctx);

    return {
        dispose() {
            cancelAnimationFrame(animationId);
            gui.destroy();
            if (currentMesh) {
                ctx.scene.remove(currentMesh.group);
                currentMesh.dispose();
            }
            if (debugCircle) {
                ctx.scene.remove(debugCircle);
                debugCircle.geometry.dispose();
                (debugCircle.material as THREE.Material).dispose();
            }
            if (axesGroup) {
                ctx.scene.remove(axesGroup);
                axesGroup.traverse((obj) => {
                    if (obj instanceof THREE.Line) {
                        obj.geometry.dispose();
                        (obj.material as THREE.Material).dispose();
                    }
                });
            }
            ctx.resizeObserver.disconnect();
            ctx.renderer.dispose();
            ctx.renderer.domElement.remove();
        }
    };
}
