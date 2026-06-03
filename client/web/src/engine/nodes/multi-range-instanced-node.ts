import * as THREE from 'three';
import { StorageInstancedBufferAttribute } from 'three/webgpu';

/** Manage instance data associated to each instance with unique keys */
class InstanceBuffer {
    readonly maxInstances: number;
    readonly floatsPerInstance: number;
    readonly data: Float32Array;
    readonly storageAttr: StorageInstancedBufferAttribute;

    readonly keyToSlot = new Map<number, number>();
    private readonly slotToKey: Uint32Array;
    private readonly live: Uint8Array;
    private freeList: number[] = [];
    private count = 0;
    private tail = 0;
    private dirty = false;

    constructor(maxInstances: number, floatsPerInstance: number) {
        this.maxInstances = maxInstances;
        this.floatsPerInstance = floatsPerInstance;
        // itemSize=3 (vec3); total elements = maxInstances * (floatsPerInstance / 3)
        this.data = new Float32Array(maxInstances * floatsPerInstance);
        this.storageAttr = new StorageInstancedBufferAttribute(this.data, 3);
        this.storageAttr.setUsage(THREE.DynamicDrawUsage);
        this.slotToKey = new Uint32Array(maxInstances);
        this.live = new Uint8Array(maxInstances);
    }

    get length(): number {
        return this.count;
    }

    get keys(): IterableIterator<number> {
        return this.keyToSlot.keys();
    }

    set(key: number, values: Float32Array): boolean {
        const existing = this.keyToSlot.get(key);
        if (existing !== undefined) {
            this.write(existing, values);
            return true;
        }

        if (this.count >= this.maxInstances) return false;
        const slot = this.freeList.length > 0 ? this.freeList.pop()! : this.tail++;
        this.slotToKey[slot] = key;
        this.live[slot] = 1;
        this.keyToSlot.set(key, slot);
        this.count++;
        if (slot < this.tail - 1) this.dirty = true;
        this.write(slot, values);
        return true;
    }

    remove(key: number): boolean {
        const slot = this.keyToSlot.get(key);
        if (slot === undefined) return false;
        this.live[slot] = 0;
        this.freeList.push(slot);
        this.keyToSlot.delete(key);
        this.count--;
        this.dirty = true;
        return true;
    }

    compact(): number {
        if (this.dirty) {
            while (this.tail > 0 && !this.live[this.tail - 1]) this.tail--;
            let lo = 0;
            while (lo < this.tail) {
                if (this.live[lo]) {
                    lo++;
                    continue;
                }
                let hi = this.tail - 1;
                while (hi > lo && !this.live[hi]) hi--;
                if (hi <= lo) break;
                this.moveSlot(hi, lo);
                this.tail = hi;
                lo++;
            }
            this.tail = this.count;
            this.freeList = [];
            this.dirty = false;
        }

        this.storageAttr.needsUpdate = true;
        return this.count;
    }

    private write(slot: number, values: Float32Array): void {
        this.data.set(values, slot * this.floatsPerInstance);
    }

    private moveSlot(src: number, dst: number): void {
        this.data.copyWithin(
            dst * this.floatsPerInstance,
            src * this.floatsPerInstance,
            (src + 1) * this.floatsPerInstance
        );
        const key = this.slotToKey[src];
        this.slotToKey[dst] = key;
        this.live[dst] = 1;
        this.live[src] = 0;
        this.keyToSlot.set(key, dst);
    }
}

class RangeMesh extends THREE.Mesh {
    readonly instanceBuffer: InstanceBuffer;

    constructor(
        sourceGeo: THREE.BufferGeometry,
        vertexStart: number,
        vertexEnd: number,
        instanceBuffer: InstanceBuffer,
        material: THREE.Material
    ) {
        // create a new geometry sharing all but instance buffer with the source geometry
        const geo = new THREE.InstancedBufferGeometry();
        for (const [name, attr] of Object.entries(sourceGeo.attributes)) geo.setAttribute(name, attr);
        geo.setIndex(sourceGeo.index);
        geo.setDrawRange(vertexStart, vertexEnd - vertexStart);
        geo.instanceCount = 0;

        super(geo, material);
        this.instanceBuffer = instanceBuffer;
        this.frustumCulled = false;
        this.onBeforeRender = (renderer, scene, camera, geometry, material, group) => {
            (this.geometry as THREE.InstancedBufferGeometry).instanceCount = this.instanceBuffer.compact();
            THREE.Mesh.prototype.onBeforeRender.call(this, renderer, scene, camera, geometry, material, group);
        };
    }

    dispose(): void {
        (this.material as THREE.Material).dispose();
    }
}

export type MultiRangeInstancedParams = {
    /** Geometry to instanace. All attributes will be shared for each range. */
    geometry: THREE.BufferGeometry;
    /** Pairs of (start, end) vertex indices defining the draw ranges. */
    ranges: Uint32Array;
    /** Number of maximum instances per range. Either a single number applied to all ranges, or an array of length equal to the number of ranges. */
    maxInstances: number | number[];
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
        const fpi = this.floatsPerInstance();
        const n = this._ranges.length >> 1;
        for (let rangeIndex = 0; rangeIndex < n; rangeIndex++) {
            const start = this._ranges[rangeIndex * 2];
            const end = this._ranges[rangeIndex * 2 + 1];
            if (end <= start) continue;
            const cap = Array.isArray(this._maxInstances) ? (this._maxInstances[rangeIndex] ?? 1) : this._maxInstances;
            const instanceBuffer = new InstanceBuffer(cap, fpi);
            const mat = this.createMaterial(rangeIndex, instanceBuffer.storageAttr);
            const mesh = new RangeMesh(this.sourceGeo, start, end, instanceBuffer, mat);
            this.group.add(mesh);
            this.rangeMeshes.push(mesh);
        }
    }

    /** Total floats stored per instance in the storage buffer. Must be a multiple of 3. */
    protected abstract floatsPerInstance(): number;

    /** Called once per range during init(). storageAttr is already allocated. */
    protected abstract createMaterial(rangeIndex: number, storageAttr: StorageInstancedBufferAttribute): THREE.Material;

    protected setInstance(rangeIndex: number, key: number, values: Float32Array): boolean {
        return this.rangeMeshes[rangeIndex]?.instanceBuffer.set(key, values) ?? false;
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
        for (const mesh of this.rangeMeshes) mesh.dispose();
        this.rangeMeshes.length = 0;
        this.group.parent?.remove(this.group);
    }
}
