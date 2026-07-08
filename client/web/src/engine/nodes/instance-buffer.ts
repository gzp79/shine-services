import * as THREE from 'three';

export function nextPow2(n: number): number {
    if (n <= 1) return 1;
    let p = 1;
    while (p < n) p <<= 1;
    return p;
}

/**
 * Manage instance data across N separate DataTexture buffers, keyed by unique instance keys.
 *
 * Each buffer is a RGBA32F DataTexture of size (maxInstances * texelsPerInstance) × 1.
 * CPU writes go into texture.image.data; setting needsUpdate = true re-uploads to GPU.
 * Buffers are independent — only dirty ones are re-uploaded each frame.
 */
export class InstanceBuffer {
    maxInstances: number;
    textures: readonly THREE.DataTexture[];
    readonly texelsPerBuffer: readonly number[];

    readonly keyToSlot = new Map<number, number>();
    private slotToKey: Uint32Array;
    private live: Uint8Array;
    private freeList: number[] = [];
    private count = 0;
    private tail = 0;
    private slotsDirty = false;
    private readonly bufferDirty: boolean[];

    constructor(maxInstances: number, texelsPerBuffer: number[]) {
        this.maxInstances = nextPow2(Math.max(1, maxInstances));
        this.texelsPerBuffer = texelsPerBuffer;
        this.textures = texelsPerBuffer.map((t) => {
            // Layout: width=texelsPerInstance, height=maxInstances
            // Slot N occupies a contiguous row: data[slot*t*4 .. (slot+1)*t*4)
            // Access in shader: ivec2(texelOffset, instanceIndex)
            // Enables contiguous subarray views per slot and partial row re-upload.
            const tex = new THREE.DataTexture(
                new Float32Array(t * this.maxInstances * 4),
                t,
                this.maxInstances,
                THREE.RGBAFormat,
                THREE.FloatType
            );
            tex.magFilter = THREE.NearestFilter;
            tex.minFilter = THREE.NearestFilter;
            tex.needsUpdate = true;
            return tex;
        });
        this.slotToKey = new Uint32Array(this.maxInstances);
        this.live = new Uint8Array(this.maxInstances);
        this.bufferDirty = texelsPerBuffer.map(() => false);
    }

    get length(): number {
        return this.count;
    }

    get keys(): IterableIterator<number> {
        return this.keyToSlot.keys();
    }

    get hasDirty(): boolean {
        return this.bufferDirty.some((d) => d);
    }

    /** Grow to newCapacity (must be > maxInstances). Returns new DataTexture array. */
    grow(newCapacity: number): THREE.DataTexture[] {
        if (newCapacity <= this.maxInstances)
            throw new Error(`grow: newCapacity ${newCapacity} must be > current ${this.maxInstances}`);

        const newTextures = (this.texelsPerBuffer as number[]).map((t, i) => {
            const oldData = this.textures[i].image.data as Float32Array;
            const newData = new Float32Array(t * newCapacity * 4);
            newData.set(oldData);
            const tex = new THREE.DataTexture(newData, t, newCapacity, THREE.RGBAFormat, THREE.FloatType);
            tex.magFilter = THREE.NearestFilter;
            tex.minFilter = THREE.NearestFilter;
            tex.needsUpdate = true;
            this.textures[i].dispose();
            return tex;
        });
        (this.textures as THREE.DataTexture[]) = newTextures;

        const oldSlotToKey = this.slotToKey;
        const oldLive = this.live;
        this.slotToKey = new Uint32Array(newCapacity);
        this.live = new Uint8Array(newCapacity);
        this.slotToKey.set(oldSlotToKey);
        this.live.set(oldLive);

        this.maxInstances = newCapacity;
        for (let i = 0; i < this.bufferDirty.length; i++) this.bufferDirty[i] = true;
        return newTextures;
    }

    setBuffer(key: number, bufIndex: number, values: Float32Array): boolean {
        const existing = this.keyToSlot.get(key);
        if (existing !== undefined) {
            console.log(`update: key=${key} slot=${existing} (buf=${bufIndex})`);
            this.writeBuffer(existing, bufIndex, values);
            this.bufferDirty[bufIndex] = true;
            return true;
        }

        if (this.count >= this.maxInstances) return false;
        const slot = this.freeList.length > 0 ? this.freeList.pop()! : this.tail++;
        this.slotToKey[slot] = key;
        this.live[slot] = 1;
        this.keyToSlot.set(key, slot);
        this.count++;
        if (slot < this.tail - 1) this.slotsDirty = true;
        console.log(`add: key=${key} → slot=${slot} (buf=${bufIndex}, count=${this.count})`);
        this.writeBuffer(slot, bufIndex, values);
        this.bufferDirty[bufIndex] = true;
        return true;
    }

    remove(key: number): boolean {
        const slot = this.keyToSlot.get(key);
        if (slot === undefined) return false;
        this.live[slot] = 0;
        this.freeList.push(slot);
        this.keyToSlot.delete(key);
        this.count--;
        this.slotsDirty = true;
        console.log(`remove: key=${key} slot=${slot} (count=${this.count})`);
        return true;
    }

    /** Compact live slots to [0, count) and return count. When slots move, all buffers are marked dirty. */
    compact(): number {
        if (this.slotsDirty) {
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
                console.log(`compact: move slot ${hi} → ${lo} (key=${this.slotToKey[hi]})`);
                this.moveSlot(hi, lo);
                this.tail = hi;
                lo++;
            }
            this.tail = this.count;
            this.freeList = [];
            this.slotsDirty = false;
            for (let i = 0; i < this.bufferDirty.length; i++) this.bufferDirty[i] = true;
        }
        return this.count;
    }

    isDirty(bufIndex: number): boolean {
        return this.bufferDirty[bufIndex];
    }

    clearDirty(bufIndex: number): void {
        this.bufferDirty[bufIndex] = false;
    }

    dispose(): void {
        for (const tex of this.textures) tex.dispose();
    }

    /** Copy slot data into `out` starting at `offset`. */
    toSlotArray(texIdx: number, slot: number, out: number[] = [], offset = 0): number[] {
        const data = this.textures[texIdx].image.data as Float32Array;
        const floats = this.texelsPerBuffer[texIdx] * 4;
        const base = slot * floats;
        for (let i = 0; i < floats; i++) out[offset + i] = data[base + i];
        return out;
    }

    private writeBuffer(slot: number, bufIndex: number, values: Float32Array): void {
        const texels = this.texelsPerBuffer[bufIndex];
        const floats = texels * 4;
        const data = this.textures[bufIndex].image.data as Float32Array;
        // Slot N occupies a contiguous row: data[slot*floats .. (slot+1)*floats)
        data.set(values.subarray(0, floats), slot * floats);
    }

    private moveSlot(src: number, dst: number): void {
        for (let i = 0; i < this.textures.length; i++) {
            const data = this.textures[i].image.data as Float32Array;
            const floats = this.texelsPerBuffer[i] * 4;
            data.copyWithin(dst * floats, src * floats, src * floats + floats);
        }
        const key = this.slotToKey[src];
        this.slotToKey[dst] = key;
        this.live[dst] = 1;
        this.live[src] = 0;
        this.keyToSlot.set(key, dst);
    }
}
