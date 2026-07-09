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
 * Two layout modes controlled by the `stripHeight` constructor parameter:
 *
 * stripHeight = 0 (default): tall texture, grows height.
 *   Texture: width=texelsPerInstance, height=maxInstances
 *   Slot N at: ivec2(texelOffset, N)
 *   Grows height when capacity exceeded. Limited by maxTextureDimension2D.
 *
 * stripHeight > 0: wide texture, grows width.
 *   Texture: width=numStrips*texelsPerInstance, height=stripHeight (fixed)
 *   Slot N: strip = N/stripHeight, row = N%stripHeight
 *   Slot N at: ivec2(strip*texelsPerInstance + texelOffset, row)
 *   Adding instances beyond stripHeight adds a new strip (wider texture, same height).
 *   Max instances = floor(maxTextureDimension2D/texelsPerInstance) * stripHeight.
 */
export class InstanceBuffer {
    maxInstances: number;
    textures: readonly THREE.DataTexture[];
    readonly texelsPerBuffer: readonly number[];
    readonly stripHeight: number;

    private numStrips: number;
    readonly keyToSlot = new Map<number, number>();
    private slotToKey: Uint32Array;
    private live: Uint8Array;
    private freeList: number[] = [];
    private count = 0;
    private tail = 0;
    private slotsDirty = false;
    private readonly bufferDirty: boolean[];
    private cpuCapacity: number;
    private cpuData: Float32Array[];

    constructor(maxInstances: number, texelsPerBuffer: number[], stripHeight = 0) {
        this.stripHeight = stripHeight;
        this.texelsPerBuffer = texelsPerBuffer;
        this.bufferDirty = texelsPerBuffer.map(() => false);

        if (stripHeight > 0) {
            this.numStrips = 1;
            this.maxInstances = stripHeight;
            this.cpuCapacity = stripHeight;
            this.cpuData = texelsPerBuffer.map((t) => new Float32Array(t * stripHeight * 4));
            this.textures = texelsPerBuffer.map((t, i) => {
                // width = numStrips(1) * t, height = stripHeight
                const tex = new THREE.DataTexture(this.cpuData[i], t, stripHeight, THREE.RGBAFormat, THREE.FloatType);
                tex.magFilter = THREE.NearestFilter;
                tex.minFilter = THREE.NearestFilter;
                tex.needsUpdate = true;
                return tex;
            });
            this.slotToKey = new Uint32Array(stripHeight);
            this.live = new Uint8Array(stripHeight);
        } else {
            this.numStrips = 0;
            this.maxInstances = nextPow2(Math.max(1, maxInstances));
            this.cpuCapacity = this.maxInstances;
            this.cpuData = texelsPerBuffer.map((t) => new Float32Array(t * this.maxInstances * 4));
            this.textures = texelsPerBuffer.map((t, i) => {
                // Layout: width=texelsPerInstance, height=maxInstances
                // Slot N occupies a contiguous row: cpuData[slot*t*4 .. (slot+1)*t*4)
                // Access in shader: ivec2(texelOffset, instanceIndex)
                const tex = new THREE.DataTexture(
                    this.cpuData[i],
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
        }
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

    /** Grow DataTextures to newCapacity (must be > maxInstances). Returns new DataTexture array. */
    grow(newCapacity: number): THREE.DataTexture[] {
        if (newCapacity <= this.maxInstances)
            throw new Error(`grow: newCapacity ${newCapacity} must be > current ${this.maxInstances}`);
        if (newCapacity > this.cpuCapacity) {
            this._growCpu(newCapacity);
        }
        const strip = this.stripHeight > 0;
        const newTextures = (this.texelsPerBuffer as number[]).map((t, i) => {
            const tex = new THREE.DataTexture(
                this.cpuData[i],
                strip ? this.numStrips * t : t,
                strip ? this.stripHeight : newCapacity,
                THREE.RGBAFormat,
                THREE.FloatType
            );
            tex.magFilter = THREE.NearestFilter;
            tex.minFilter = THREE.NearestFilter;
            tex.needsUpdate = true;
            this.textures[i].dispose();
            return tex;
        });
        (this.textures as THREE.DataTexture[]) = newTextures;
        this.maxInstances = strip ? this.cpuCapacity : newCapacity;
        for (let i = 0; i < this.bufferDirty.length; i++) this.bufferDirty[i] = true;
        return newTextures;
    }

    setBuffer(key: number, bufIndex: number, values: Float32Array): boolean {
        const existing = this.keyToSlot.get(key);
        if (existing !== undefined) {
            this.writeBuffer(existing, bufIndex, values);
            this.bufferDirty[bufIndex] = true;
            return true;
        }

        if (this.count >= this.cpuCapacity) {
            if (this.stripHeight > 0) {
                this._growCpu(this.cpuCapacity + this.stripHeight); // add one strip
            } else {
                this._growCpu(this.cpuCapacity * 2);
            }
        }
        const slot = this.freeList.length > 0 ? this.freeList.pop()! : this.tail++;
        this.slotToKey[slot] = key;
        this.live[slot] = 1;
        this.keyToSlot.set(key, slot);
        this.count++;
        if (slot < this.tail - 1) this.slotsDirty = true;
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
        return true;
    }

    /**
     * Compact live slots to [0, count). Grows DataTextures if CPU capacity exceeded.
     * Returns true if DataTextures were recreated (callers must recreate materials).
     */
    compact(): boolean {
        let grew = false;
        if (this.cpuCapacity > this.maxInstances) {
            this.grow(this.cpuCapacity);
            grew = true;
        }
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
                this.moveSlot(hi, lo);
                this.tail = hi;
                lo++;
            }
            this.tail = this.count;
            this.freeList = [];
            this.slotsDirty = false;
            for (let i = 0; i < this.bufferDirty.length; i++) this.bufferDirty[i] = true;
        }
        return grew;
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
        const data = this.cpuData[texIdx];
        const T = this.texelsPerBuffer[texIdx];
        const floats = T * 4;
        const base = this.stripHeight > 0 ? this._stripOffset(texIdx, slot) : slot * floats;
        for (let i = 0; i < floats; i++) out[offset + i] = data[base + i];
        return out;
    }

    private _stripOffset(bufIndex: number, slot: number): number {
        const T = this.texelsPerBuffer[bufIndex];
        const strip = Math.floor(slot / this.stripHeight);
        const row = slot % this.stripHeight;
        return (row * this.numStrips * T + strip * T) * 4;
    }

    private _growCpu(newCap: number): void {
        if (this.stripHeight > 0) {
            const newNumStrips = Math.ceil(newCap / this.stripHeight);
            const actualNewCap = newNumStrips * this.stripHeight;
            const oldNumStrips = this.numStrips;
            this.cpuData = (this.texelsPerBuffer as number[]).map((T, i) => {
                const oldData = this.cpuData[i];
                const newData = new Float32Array(newNumStrips * T * this.stripHeight * 4);
                // Rearrange strip layout: row stride changes from oldNumStrips*T to newNumStrips*T
                for (let row = 0; row < this.stripHeight; row++) {
                    const rowOldBase = row * oldNumStrips * T * 4;
                    newData.set(
                        oldData.subarray(rowOldBase, rowOldBase + oldNumStrips * T * 4),
                        row * newNumStrips * T * 4
                    );
                }
                return newData;
            });
            this.numStrips = newNumStrips;
            this._growSlotArrays(actualNewCap);
            this.cpuCapacity = actualNewCap;
        } else {
            this.cpuData = (this.texelsPerBuffer as number[]).map((t, i) => {
                const newData = new Float32Array(t * newCap * 4);
                newData.set(this.cpuData[i]);
                return newData;
            });
            this._growSlotArrays(newCap);
            this.cpuCapacity = newCap;
        }
    }

    private _growSlotArrays(newCap: number): void {
        if (newCap <= this.slotToKey.length) return;
        const newSlotToKey = new Uint32Array(newCap);
        const newLive = new Uint8Array(newCap);
        newSlotToKey.set(this.slotToKey);
        newLive.set(this.live);
        this.slotToKey = newSlotToKey;
        this.live = newLive;
    }

    private writeBuffer(slot: number, bufIndex: number, values: Float32Array): void {
        const T = this.texelsPerBuffer[bufIndex];
        const floats = T * 4;
        const data = this.cpuData[bufIndex];
        const base = this.stripHeight > 0 ? this._stripOffset(bufIndex, slot) : slot * floats;
        data.set(values.subarray(0, floats), base);
    }

    private moveSlot(src: number, dst: number): void {
        for (let i = 0; i < this.cpuData.length; i++) {
            const data = this.cpuData[i];
            const T = this.texelsPerBuffer[i];
            const floats = T * 4;
            if (this.stripHeight > 0) {
                const srcOff = this._stripOffset(i, src);
                const dstOff = this._stripOffset(i, dst);
                data.set(data.subarray(srcOff, srcOff + floats), dstOff);
            } else {
                data.copyWithin(dst * floats, src * floats, src * floats + floats);
            }
        }
        const key = this.slotToKey[src];
        this.slotToKey[dst] = key;
        this.live[dst] = 1;
        this.live[src] = 0;
        this.keyToSlot.set(key, dst);
    }
}
