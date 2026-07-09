import GUI from 'lil-gui';
import * as THREE from 'three';
import { WebGPURenderer } from 'three/webgpu';
import { MeshStandardNodeMaterial } from 'three/webgpu';
import { InstancedColorMesh } from '../../engine/nodes/instanced-color-mesh';
import { own, share } from '../../engine/render/ownership';
import { Experiment } from '../experiment';

export interface InstancedColorMeshExperiment {
    dispose(): void;
}

const PALETTE = [
    new THREE.Color(0xff3333), // red
    new THREE.Color(0xff9933), // orange
    new THREE.Color(0xffff33), // yellow
    new THREE.Color(0x33ff33), // green
    new THREE.Color(0x33ffff), // cyan
    new THREE.Color(0x3333ff), // blue
    new THREE.Color(0xcc33ff), // violet
    new THREE.Color(0xffffff) // white
];

const BOX_HALF = 5.0;

function randomTransform(): THREE.Matrix4 {
    const x = (Math.random() * 2 - 1) * BOX_HALF;
    const y = (Math.random() * 2 - 1) * BOX_HALF;
    const z = (Math.random() * 2 - 1) * BOX_HALF;
    return new THREE.Matrix4().makeTranslation(x, y, z);
}

function buildGeometry(): { geometry: THREE.BufferGeometry; ranges: number[] } {
    const sphere = new THREE.SphereGeometry(0.08, 16, 12);
    const cone = new THREE.ConeGeometry(0.06, 0.16, 16);
    const torus = new THREE.TorusGeometry(0.06, 0.02, 12, 24);
    const geos = [sphere, cone, torus];

    let totalVerts = 0;
    let totalIndices = 0;
    const vertCounts: number[] = [];

    for (const g of geos) {
        const vc = g.attributes.position.count;
        vertCounts.push(vc);
        totalVerts += vc;
        totalIndices += g.index!.count;
    }

    const positions = new Float32Array(totalVerts * 3);
    const normals = new Float32Array(totalVerts * 3);
    const indices = new Uint32Array(totalIndices);
    const ranges: number[] = [];

    let vOffset = 0;
    let iOffset = 0;

    for (let g = 0; g < geos.length; g++) {
        const geo = geos[g];
        positions.set(geo.attributes.position.array as Float32Array, vOffset * 3);
        normals.set(geo.attributes.normal.array as Float32Array, vOffset * 3);

        const srcIdx = geo.index!.array;
        ranges.push(iOffset);
        for (let i = 0; i < srcIdx.length; i++) {
            indices[iOffset + i] = srcIdx[i] + vOffset;
        }
        iOffset += srcIdx.length;
        ranges.push(iOffset);

        vOffset += vertCounts[g];
    }

    for (const g of geos) g.dispose();

    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
    geometry.setAttribute('normal', new THREE.BufferAttribute(normals, 3));
    geometry.setIndex(new THREE.BufferAttribute(indices, 1));

    return { geometry, ranges };
}

class InstancedColorMeshExp extends Experiment {
    private readonly mesh: InstancedColorMesh;
    private readonly gui: GUI;
    private readonly params = { a: 5, b: 5, c: 5 };
    private readonly counts = [0, 0, 0];

    constructor(container: HTMLElement, renderer: WebGPURenderer) {
        super(container, renderer);

        this.camera.position.set(0, -18, 12);
        this.camera.lookAt(0, 0, 0);
        if (this.controls) {
            this.controls.target.set(0, 0, 0);
            this.controls.update();
        }

        const { geometry, ranges } = buildGeometry();

        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const maxDim: number = (renderer.backend as any).device?.limits?.maxTextureDimension2D ?? 8192;

        this.mesh = new InstancedColorMesh(this.scene, {
            geometry: own(geometry),
            variants: [
                {
                    parts: [
                        {
                            baseMaterial: share(new MeshStandardNodeMaterial({ roughness: 0.6, metalness: 0.2 })),
                            indexStart: ranges[0],
                            indexEnd: ranges[1]
                        }
                    ]
                },
                {
                    parts: [
                        {
                            baseMaterial: share(new MeshStandardNodeMaterial({ roughness: 0.5, metalness: 0.3 })),
                            indexStart: ranges[2],
                            indexEnd: ranges[3]
                        }
                    ]
                },
                {
                    parts: [
                        {
                            baseMaterial: share(new MeshStandardNodeMaterial({ roughness: 0.4, metalness: 0.4 })),
                            indexStart: ranges[4],
                            indexEnd: ranges[5]
                        }
                    ]
                }
            ],
            instanceCountHint: 1,
            pageSizeHint: maxDim
        });

        this.gui = new GUI({ title: 'Instanced Color Mesh', container });
        this.gui.domElement.style.cssText = 'position:absolute;top:0;right:0;z-index:10';
        this.gui
            .add(this.params, 'a')
            .name('Spheres')
            .min(0)
            .step(1)
            .onChange((v: number) => this.update(0, v));
        this.gui
            .add(this.params, 'b')
            .name('Cones')
            .min(0)
            .step(1)
            .onChange((v: number) => this.update(1, v));
        this.gui
            .add(this.params, 'c')
            .name('Tori')
            .min(0)
            .step(1)
            .onChange((v: number) => this.update(2, v));

        this.update(0, this.params.a);
        this.update(1, this.params.b);
        this.update(2, this.params.c);

        this.start();
    }

    private update(variantIndex: number, newCount: number): void {
        const current = this.counts[variantIndex];
        for (let i = newCount; i < current; i++) {
            this.mesh.removeObject(variantIndex, variantIndex * 100_000 + i);
        }
        for (let i = current; i < newCount; i++) {
            const key = variantIndex * 100_000 + i;
            const matrix = randomTransform();
            const color = PALETTE[i % PALETTE.length];
            this.mesh.setObject(variantIndex, key, matrix, color);
        }
        this.counts[variantIndex] = newCount;
    }

    dispose(): void {
        this.gui.destroy();
        this.mesh.dispose();
        super.dispose();
    }
}

export async function createInstancedColorMeshExperiment(
    container: HTMLElement,
    renderer: WebGPURenderer
): Promise<InstancedColorMeshExperiment> {
    return new InstancedColorMeshExp(container, renderer);
}
