import * as THREE from 'three';
import { FBXLoader } from 'three/addons/loaders/FBXLoader.js';
import { attribute, mix, uniform } from 'three/tsl';
import { MeshStandardNodeMaterial } from 'three/webgpu';
import { ControlBox } from '../../engine/utils';
import {
    type ExperimentContext,
    animate,
    createExperiment,
    disposeExperiment,
    disposeMesh,
    disposeObject3D
} from '../experiment';

export interface TrilinearExperiment {
    dispose(): void;
}

export async function createTrilinearExperiment(container: HTMLElement): Promise<TrilinearExperiment> {
    const ctx: ExperimentContext = await createExperiment(container);

    // File input UI
    const fileInput = document.createElement('input');
    fileInput.type = 'file';
    fileInput.accept = '.fbx';
    fileInput.style.position = 'absolute';
    fileInput.style.top = '50px';
    fileInput.style.left = '10px';
    fileInput.style.zIndex = '100';
    fileInput.style.padding = '8px';
    fileInput.style.background = 'rgba(0, 0, 0, 0.8)';
    fileInput.style.color = 'white';
    fileInput.style.border = '1px solid #555';
    fileInput.style.borderRadius = '4px';
    fileInput.style.fontFamily = 'monospace';
    container.appendChild(fileInput);

    let loadedMesh: THREE.Mesh | null = null;
    let loadedObject: THREE.Group | null = null;

    const controlBox = new ControlBox({
        scene: ctx.scene,
        camera: ctx.camera,
        domElement: ctx.renderer.domElement,
        onDragStart: () => {
            ctx.controls!.enabled = false;
        },
        onDragEnd: () => {
            ctx.controls!.enabled = true;
        }
    });

    // Helper: Check if mesh is compatible with trilinear interpolation
    function validateMeshForTrilinear(mesh: THREE.Mesh): { valid: boolean; reason?: string } {
        if (mesh instanceof THREE.SkinnedMesh) {
            return { valid: false, reason: 'SkinnedMesh with skeletal animation' };
        }
        if (mesh.geometry.morphAttributes.position) {
            return { valid: false, reason: 'Morph targets (shape keys)' };
        }
        if (mesh.geometry.attributes.skinIndex || mesh.geometry.attributes.skinWeight) {
            return { valid: false, reason: 'Skinning attributes' };
        }

        return { valid: true };
    }

    function bakeTransformIntoGeometry(mesh: THREE.Mesh): void {
        mesh.updateMatrixWorld(true);
        mesh.geometry.applyMatrix4(mesh.matrixWorld);

        mesh.geometry.boundingBox = null;
        mesh.geometry.boundingSphere = null;

        if (mesh.geometry.attributes.position) {
            mesh.geometry.attributes.position.needsUpdate = true;
        }

        mesh.position.set(0, 0, 0);
        mesh.rotation.set(0, 0, 0);
        mesh.scale.set(1, 1, 1);
        mesh.updateMatrix();
    }

    function convertToTrilinearCoordinates(geometry: THREE.BufferGeometry, bbox: THREE.Box3): void {
        const min = bbox.min;
        const max = bbox.max;
        const size = new THREE.Vector3().subVectors(max, min);

        const positions = geometry.attributes.position;
        for (let i = 0; i < positions.count; i++) {
            const x = positions.getX(i);
            const y = positions.getY(i);
            const z = positions.getZ(i);

            // Convert to 0-1 trilinear space
            positions.setXYZ(i, (x - min.x) / size.x, (y - min.y) / size.y, (z - min.z) / size.z);
        }

        positions.needsUpdate = true;
        geometry.boundingBox = null;
        geometry.boundingSphere = null;
    }

    function createTrilinearMaterial(): MeshStandardNodeMaterial {
        const material = new MeshStandardNodeMaterial({
            color: 0x4a9eff,
            metalness: 0.3,
            roughness: 0.6,
            side: THREE.DoubleSide
        });

        // Create individual uniforms for each control point
        const corners = controlBox.corners;
        const cp0 = uniform(corners[0]);
        const cp1 = uniform(corners[1]);
        const cp2 = uniform(corners[2]);
        const cp3 = uniform(corners[3]);
        const cp4 = uniform(corners[4]);
        const cp5 = uniform(corners[5]);
        const cp6 = uniform(corners[6]);
        const cp7 = uniform(corners[7]);

        const pos = attribute('position', 'vec3');
        const c00 = mix(cp0, cp1, pos.x);
        const c01 = mix(cp2, cp3, pos.x);
        const c10 = mix(cp4, cp5, pos.x);
        const c11 = mix(cp6, cp7, pos.x);
        const c0 = mix(c00, c01, pos.y);
        const c1 = mix(c10, c11, pos.y);
        const deformedPosition = mix(c0, c1, pos.z);
        material.positionNode = deformedPosition;

        return material;
    }

    function disposeLoadedObject(): void {
        if (loadedObject) {
            disposeObject3D(loadedObject);
            ctx.scene.remove(loadedObject);
            loadedMesh = null;
            loadedObject = null;
        }
    }

    function createDefaultMesh(): THREE.Mesh {
        disposeLoadedObject();
        const geometry = new THREE.BoxGeometry(3, 3, 3, 16, 16, 16);
        geometry.computeBoundingBox();
        convertToTrilinearCoordinates(geometry, geometry.boundingBox!);
        const material = createTrilinearMaterial();
        return new THREE.Mesh(geometry, material);
    }

    async function createMeshFromFile(file: File) {
        disposeLoadedObject();

        const fileExt = file.name.split('.').pop()?.toLowerCase();
        try {
            if (fileExt === 'fbx') {
                const buffer = await file.arrayBuffer();
                const loader = new FBXLoader();
                loadedObject = loader.parse(buffer, '');
            } else {
                const msg = 'Unsupported file format. Please use .fbx';
                console.error(msg);
                alert(msg);
                return;
            }

            // Get all meshes from loaded object
            const meshes: THREE.Mesh[] = [];
            loadedObject.traverse((child) => {
                if (child instanceof THREE.Mesh) {
                    meshes.push(child);
                }
            });

            if (meshes.length === 0) {
                const msg = 'No mesh found in file';
                console.error(msg);
                alert(msg);
                return;
            }

            console.log(`Loaded ${meshes.length} mesh(es) from file:`);
            meshes.forEach((mesh, i) => {
                console.log(`  [${i}] "${mesh.name}" (${mesh.geometry.attributes.position.count} vertices)`);
            });

            const filteredMeshes: THREE.Mesh[] = [];
            const rejectedMeshes: THREE.Mesh[] = [];

            meshes.forEach((mesh, i) => {
                if ([0].includes(i)) {
                    filteredMeshes.push(mesh);
                } else {
                    rejectedMeshes.push(mesh);
                }
            });

            if (filteredMeshes.length === 0) {
                const msg = 'No mesh found after filtering';
                console.error(msg);
                alert(msg);
                return;
            }
            console.log(`Using ${filteredMeshes.length} mesh(es) after filtering`);

            // Dispose rejected meshes immediately
            rejectedMeshes.forEach((mesh) => {
                mesh.parent?.remove(mesh);
                disposeMesh(mesh);
            });

            // Remove filtered meshes from original hierarchy to create clean structure
            filteredMeshes.forEach((mesh) => {
                mesh.parent?.remove(mesh);
            });

            // Dispose remaining FBX hierarchy (empties, lights, cameras, etc.)
            if (loadedObject) {
                disposeObject3D(loadedObject);
                loadedObject.clear(); // Remove all remaining children
            }

            // Create new clean group with only filtered meshes
            loadedObject = new THREE.Group();

            // Validate all meshes for trilinear compatibility
            for (let i = 0; i < filteredMeshes.length; i++) {
                const validation = validateMeshForTrilinear(filteredMeshes[i]);
                if (!validation.valid) {
                    console.error(`Mesh ${i} is incompatible: ${validation.reason}`);
                    alert(
                        `File rejected: Mesh ${i} contains ${validation.reason}\n\n` +
                            'Trilinear deformation only supports static meshes.\n' +
                            'Please export without:\n' +
                            '- Skeletal animation (armatures/bones)\n' +
                            '- Morph targets (shape keys)\n' +
                            '- Skinning data'
                    );
                    return;
                }
            }

            // Bake transforms
            filteredMeshes.forEach((mesh) => {
                bakeTransformIntoGeometry(mesh);
            });

            const combinedBox = new THREE.Box3();
            filteredMeshes.forEach((mesh) => {
                mesh.geometry.computeBoundingBox();
                if (mesh.geometry.boundingBox) {
                    combinedBox.union(mesh.geometry.boundingBox);
                }
            });

            filteredMeshes.forEach((mesh, i) => {
                if (Array.isArray(mesh.material)) {
                    mesh.material.forEach((mat) => mat.dispose());
                } else {
                    mesh.material?.dispose();
                }

                console.log(`Converting mesh ${i} to trilinear coordinates`);
                convertToTrilinearCoordinates(mesh.geometry, combinedBox);
                mesh.material = createTrilinearMaterial();

                loadedObject!.add(mesh);
            });

            ctx.scene.add(loadedObject);
            loadedMesh = filteredMeshes[0]; // Keep reference for cleanup
        } catch (error) {
            const msg = `Failed to load file: ${error}`;
            console.error(msg);
            alert(msg);
        }
    }

    loadedMesh = createDefaultMesh();
    loadedObject = new THREE.Group();
    loadedObject.add(loadedMesh);
    ctx.scene.add(loadedObject);

    fileInput.addEventListener('change', async (e) => {
        const file = (e.target as HTMLInputElement).files?.[0];
        if (!file) return;
        await createMeshFromFile(file);
    });

    const animationId = animate(ctx);

    return {
        dispose() {
            cancelAnimationFrame(animationId);
            disposeLoadedObject();
            controlBox.dispose();
            fileInput.remove();
            disposeExperiment(ctx);
        }
    };
}
