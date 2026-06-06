import * as THREE from 'three';
import { InstanceBuffer } from './instance-buffer';

class RangeMesh extends THREE.Mesh {
    readonly instanceBuffer: InstanceBuffer;

    constructor(
        sourceGeo: THREE.BufferGeometry,
        vertexStart: number,
        vertexEnd: number,
        instanceBuffer: InstanceBuffer,
        material: THREE.Material
    ) {
        const geo = new THREE.InstancedBufferGeometry();
        for (const [name, attr] of Object.entries(sourceGeo.attributes)) geo.setAttribute(name, attr);
        geo.setIndex(sourceGeo.index);
        geo.setDrawRange(vertexStart, vertexEnd - vertexStart);
        geo.instanceCount = 0;

        super(geo, material);
        this.instanceBuffer = instanceBuffer;
        this.frustumCulled = false;
        this.onBeforeRender = (renderer, scene, camera, geometry, material, group) => {
            const count = this.instanceBuffer.compact();
            (this.geometry as THREE.InstancedBufferGeometry).instanceCount = count;
            for (let i = 0; i < instanceBuffer.textures.length; i++) {
                if (this.instanceBuffer.isDirty(i)) {
                    this.instanceBuffer.textures[i].needsUpdate = true;
                    this.instanceBuffer.clearDirty(i);
                }
            }
            THREE.Mesh.prototype.onBeforeRender.call(this, renderer, scene, camera, geometry, material, group);
        };
    }

    dispose(): void {
        (this.material as THREE.Material).dispose();
    }
}

export type MultiRangeInstancedParams = {
    /** Geometry to instance. All attributes will be shared for each range. */
    geometry: THREE.BufferGeometry;
    /** Pairs of (start, end) vertex indices defining the draw ranges. */
    ranges: Uint32Array;
    /** Number of maximum instances per range. Either a single number applied to all ranges, or an array of length equal to the number of ranges. */
    maxInstances: number | number[];
};

/**
 * Per-instance data layout across N DataTexture buffers.
 *
 * Each buffer is a RGBA32F texture — only dirty buffers are re-uploaded.
 *   - `floatsPerInstance`: floats written per instance (must be a multiple of 4)
 *   - `isDynamic`: hint for update frequency (default true); currently informational only
 */
export type InstanceBufferLayout = {
    buffers: Array<{ floatsPerInstance: number; isDynamic?: boolean }>;
};

export abstract class MultiRangeInstancedNode {
    readonly group = new THREE.Group();
    private readonly rangeMeshes: RangeMesh[] = [];
    protected readonly sourceGeo: THREE.BufferGeometry;
    private readonly _ranges: Uint32Array;
    private readonly _maxInstances: number | number[];

    protected constructor(parent: THREE.Object3D, params: MultiRangeInstancedParams) {
        parent.add(this.group);
        this.sourceGeo = params.geometry;
        this._ranges = params.ranges;
        this._maxInstances = params.maxInstances;
    }

    protected init(): void {
        const layout = this.instanceBufferLayout();
        const n = this._ranges.length >> 1;
        for (let rangeIndex = 0; rangeIndex < n; rangeIndex++) {
            const start = this._ranges[rangeIndex * 2];
            const end = this._ranges[rangeIndex * 2 + 1];
            if (end <= start) continue;
            const cap = Array.isArray(this._maxInstances) ? (this._maxInstances[rangeIndex] ?? 1) : this._maxInstances;
            const texelsPerBuffer = layout.buffers.map((b) => {
                if (b.floatsPerInstance % 4 !== 0)
                    throw new Error(`floatsPerInstance (${b.floatsPerInstance}) must be a multiple of 4`);
                return b.floatsPerInstance / 4;
            });
            const instanceBuffer = new InstanceBuffer(cap, texelsPerBuffer);
            const mat = this.createMaterial(rangeIndex, instanceBuffer.textures);
            const mesh = new RangeMesh(this.sourceGeo, start, end, instanceBuffer, mat);
            this.group.add(mesh);
            this.rangeMeshes.push(mesh);
        }
    }

    protected abstract instanceBufferLayout(): InstanceBufferLayout;

    /** Called once per range during init(). textures[i] corresponds to layout.buffers[i]. */
    protected abstract createMaterial(rangeIndex: number, textures: readonly THREE.DataTexture[]): THREE.Material;

    protected setInstanceBuffer(rangeIndex: number, key: number, bufIndex: number, values: Float32Array): boolean {
        return this.rangeMeshes[rangeIndex]?.instanceBuffer.setBuffer(key, bufIndex, values) ?? false;
    }

    removeInstance(rangeIndex: number, key: number): boolean {
        return this.rangeMeshes[rangeIndex]?.instanceBuffer.remove(key) ?? false;
    }

    instanceCount(rangeIndex: number): number {
        return this.rangeMeshes[rangeIndex]?.instanceBuffer.length ?? 0;
    }

    instances(rangeIndex: number): IterableIterator<number> {
        return this.rangeMeshes[rangeIndex]?.instanceBuffer.keys ?? [][Symbol.iterator]();
    }

    dispose(): void {
        for (const mesh of this.rangeMeshes) {
            mesh.instanceBuffer.dispose();
            mesh.dispose();
        }
        this.rangeMeshes.length = 0;
        this.group.parent?.remove(this.group);
    }
}
