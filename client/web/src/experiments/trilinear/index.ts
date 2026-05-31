import * as THREE from 'three';
import { FBXLoader } from 'three/addons/loaders/FBXLoader.js';
import { attribute, mix, uniform } from 'three/tsl';
import { MeshStandardNodeMaterial, WebGPURenderer } from 'three/webgpu';
import { ControlBox } from '../../engine/utils';
import { Experiment, disposeMesh, disposeObject3D } from '../experiment';

export interface TrilinearExperiment {
    dispose(): void;
}

class Trilinear extends Experiment {
    private loadedMesh: THREE.Mesh | null = null;
    private loadedObject: THREE.Group | null = null;
    private readonly controlBox: ControlBox;
    private readonly fileInput: HTMLInputElement;

    constructor(container: HTMLElement, renderer: WebGPURenderer) {
        super(container, renderer);

        this.fileInput = document.createElement('input');
        this.fileInput.type = 'file';
        this.fileInput.accept = '.fbx';
        this.fileInput.style.position = 'absolute';
        this.fileInput.style.top = '50px';
        this.fileInput.style.left = '10px';
        this.fileInput.style.zIndex = '100';
        this.fileInput.style.padding = '8px';
        this.fileInput.style.background = 'rgba(0, 0, 0, 0.8)';
        this.fileInput.style.color = 'white';
        this.fileInput.style.border = '1px solid #555';
        this.fileInput.style.borderRadius = '4px';
        this.fileInput.style.fontFamily = 'monospace';
        container.appendChild(this.fileInput);

        this.controlBox = new ControlBox({
            scene: this.scene,
            camera: this.camera,
            domElement: this.renderer.domElement,
            onDragStart: () => {
                this.controls!.enabled = false;
            },
            onDragEnd: () => {
                this.controls!.enabled = true;
            }
        });

        this.loadedMesh = this.createDefaultMesh();
        this.loadedObject = new THREE.Group();
        this.loadedObject.add(this.loadedMesh);
        this.scene.add(this.loadedObject);

        this.fileInput.addEventListener('change', async (e) => {
            const file = (e.target as HTMLInputElement).files?.[0];
            if (!file) return;
            await this.createMeshFromFile(file);
        });

        this.start();
    }

    private validateMeshForTrilinear(mesh: THREE.Mesh): { valid: boolean; reason?: string } {
        if (mesh instanceof THREE.SkinnedMesh) return { valid: false, reason: 'SkinnedMesh with skeletal animation' };
        if (mesh.geometry.morphAttributes.position) return { valid: false, reason: 'Morph targets (shape keys)' };
        if (mesh.geometry.attributes.skinIndex || mesh.geometry.attributes.skinWeight) {
            return { valid: false, reason: 'Skinning attributes' };
        }
        return { valid: true };
    }

    private bakeTransformIntoGeometry(mesh: THREE.Mesh): void {
        mesh.updateMatrixWorld(true);
        mesh.geometry.applyMatrix4(mesh.matrixWorld);
        mesh.geometry.boundingBox = null;
        mesh.geometry.boundingSphere = null;
        if (mesh.geometry.attributes.position) mesh.geometry.attributes.position.needsUpdate = true;
        mesh.position.set(0, 0, 0);
        mesh.rotation.set(0, 0, 0);
        mesh.scale.set(1, 1, 1);
        mesh.updateMatrix();
    }

    private convertToTrilinearCoordinates(geometry: THREE.BufferGeometry, bbox: THREE.Box3): void {
        const min = bbox.min;
        const max = bbox.max;
        const size = new THREE.Vector3().subVectors(max, min);
        const positions = geometry.attributes.position;
        for (let i = 0; i < positions.count; i++) {
            positions.setXYZ(
                i,
                (positions.getX(i) - min.x) / size.x,
                (positions.getY(i) - min.y) / size.y,
                (positions.getZ(i) - min.z) / size.z
            );
        }
        positions.needsUpdate = true;
        geometry.boundingBox = null;
        geometry.boundingSphere = null;
    }

    private createTrilinearMaterial(): MeshStandardNodeMaterial {
        const material = new MeshStandardNodeMaterial({
            color: 0x4a9eff,
            metalness: 0.3,
            roughness: 0.6,
            side: THREE.DoubleSide
        });
        const corners = this.controlBox.corners;
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
        material.positionNode = mix(c0, c1, pos.z);
        return material;
    }

    private disposeLoadedObject(): void {
        if (this.loadedObject) {
            disposeObject3D(this.loadedObject);
            this.scene.remove(this.loadedObject);
            this.loadedMesh = null;
            this.loadedObject = null;
        }
    }

    private createDefaultMesh(): THREE.Mesh {
        this.disposeLoadedObject();
        const geometry = new THREE.BoxGeometry(3, 3, 3, 16, 16, 16);
        geometry.computeBoundingBox();
        this.convertToTrilinearCoordinates(geometry, geometry.boundingBox!);
        return new THREE.Mesh(geometry, this.createTrilinearMaterial());
    }

    private async createMeshFromFile(file: File) {
        this.disposeLoadedObject();

        const fileExt = file.name.split('.').pop()?.toLowerCase();
        try {
            if (fileExt === 'fbx') {
                const buffer = await file.arrayBuffer();
                const loader = new FBXLoader();
                this.loadedObject = loader.parse(buffer, '');
            } else {
                const msg = 'Unsupported file format. Please use .fbx';
                console.error(msg);
                alert(msg);
                return;
            }

            const meshes: THREE.Mesh[] = [];
            this.loadedObject.traverse((child) => {
                if (child instanceof THREE.Mesh) meshes.push(child);
            });

            if (meshes.length === 0) {
                alert('No mesh found in file');
                return;
            }

            console.log(`Loaded ${meshes.length} mesh(es) from file:`);
            meshes.forEach((mesh, i) => {
                console.log(`  [${i}] "${mesh.name}" (${mesh.geometry.attributes.position.count} vertices)`);
            });

            const filteredMeshes = meshes.filter((_, i) => [0].includes(i));
            const rejectedMeshes = meshes.filter((_, i) => ![0].includes(i));

            if (filteredMeshes.length === 0) {
                alert('No mesh found after filtering');
                return;
            }

            rejectedMeshes.forEach((mesh) => {
                mesh.parent?.remove(mesh);
                disposeMesh(mesh);
            });
            filteredMeshes.forEach((mesh) => mesh.parent?.remove(mesh));

            if (this.loadedObject) {
                disposeObject3D(this.loadedObject);
                this.loadedObject.clear();
            }
            this.loadedObject = new THREE.Group();

            for (let i = 0; i < filteredMeshes.length; i++) {
                const validation = this.validateMeshForTrilinear(filteredMeshes[i]);
                if (!validation.valid) {
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

            filteredMeshes.forEach((mesh) => this.bakeTransformIntoGeometry(mesh));

            const combinedBox = new THREE.Box3();
            filteredMeshes.forEach((mesh) => {
                mesh.geometry.computeBoundingBox();
                if (mesh.geometry.boundingBox) combinedBox.union(mesh.geometry.boundingBox);
            });

            filteredMeshes.forEach((mesh, i) => {
                if (Array.isArray(mesh.material)) {
                    mesh.material.forEach((mat) => mat.dispose());
                } else {
                    mesh.material?.dispose();
                }
                console.log(`Converting mesh ${i} to trilinear coordinates`);
                this.convertToTrilinearCoordinates(mesh.geometry, combinedBox);
                mesh.material = this.createTrilinearMaterial();
                this.loadedObject!.add(mesh);
            });

            this.scene.add(this.loadedObject);
            this.loadedMesh = filteredMeshes[0];
        } catch (error) {
            const msg = `Failed to load file: ${error}`;
            console.error(msg);
            alert(msg);
        }
    }

    dispose() {
        this.disposeLoadedObject();
        this.controlBox.dispose();
        this.fileInput.remove();
        super.dispose();
    }
}

export async function createTrilinearExperiment(
    container: HTMLElement,
    renderer: WebGPURenderer
): Promise<TrilinearExperiment> {
    return new Trilinear(container, renderer);
}
