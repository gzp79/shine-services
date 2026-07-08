import GUI from 'lil-gui';
import * as THREE from 'three';
import { WebGPURenderer } from 'three/webgpu';
import { InstancedColorMesh } from '../../engine/nodes/instanced-color-mesh';
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

const MAX_ROW = 10;
const SPACING = 1.2;
const LANE_GAP = 3.0;

function makeTranslation(variantIndex: number, instanceIndex: number): THREE.Matrix4 {
    const x = (instanceIndex % MAX_ROW) * SPACING;
    const y = variantIndex * LANE_GAP;
    const z = Math.floor(instanceIndex / MAX_ROW) * SPACING;
    return new THREE.Matrix4().makeTranslation(x, y, z);
}

function buildGeometry(): { geometry: THREE.BufferGeometry; ranges: number[] } {
    const sphere = new THREE.SphereGeometry(0.4, 16, 12);
    const cone = new THREE.ConeGeometry(0.3, 0.8, 16);
    const torus = new THREE.TorusGeometry(0.3, 0.1, 12, 24);
    const geos = [sphere, cone, torus];

    let totalVerts = 0;
    let totalIndices = 0;
    const vertCounts: number[] = [];
    const idxCounts: number[] = [];

    for (const g of geos) {
        const vc = g.attributes.position.count;
        const ic = g.index!.count;
        vertCounts.push(vc);
        idxCounts.push(ic);
        totalVerts += vc;
        totalIndices += ic;
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

        this.camera.position.set(6, 14, 16);
        this.camera.lookAt(6, 4, 3);
        if (this.controls) {
            this.controls.target.set(6, 4, 3);
            this.controls.update();
        }

        const { geometry, ranges } = buildGeometry();

        this.mesh = new InstancedColorMesh(this.scene, {
            geometry,
            variants: [
                { parts: [{ materialName: 'sphere', indexStart: ranges[0], indexEnd: ranges[1] }] },
                { parts: [{ materialName: 'cone', indexStart: ranges[2], indexEnd: ranges[3] }] },
                { parts: [{ materialName: 'torus', indexStart: ranges[4], indexEnd: ranges[5] }] }
            ],
            instanceCountHint: 1
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
            const matrix = makeTranslation(variantIndex, i);
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
