import { describe, expect, it } from 'vitest';
import { InstanceBuffer, nextPow2 } from './instance-buffer';

// 1 texel = 4 floats (RGBA32F); storing key, key*10, key*100 in first 3 components
function buf1(maxInstances: number): InstanceBuffer {
    return new InstanceBuffer(maxInstances, [1]);
}

function val(key: number): Float32Array {
    return new Float32Array([key, key * 10, key * 100, 0]);
}

function expectSlots(buf: InstanceBuffer, keys: number[]): void {
    expect(buf.length).toBe(keys.length);
    expect(buf.keyToSlot.size, 'keyToSlot size').toBe(keys.length);

    const seenSlots = new Set<number>();
    for (const key of keys) {
        const slot = buf.keyToSlot.get(key);
        expect(slot, `key ${key} missing from keyToSlot`).toBeDefined();
        expect(slot!, `key ${key} slot out of active range`).toBeLessThan(keys.length);
        expect(seenSlots.has(slot!), `slot ${slot} used by more than one key`).toBe(false);
        seenSlots.add(slot!);
        const d = buf.toSlotArray(0, slot!);
        expect(d[0]).toBe(key);
        expect(d[1]).toBe(key * 10);
        expect(d[2]).toBe(key * 100);
    }
}

describe('nextPow2', () => {
    it('returns 1 for 0 and 1', () => {
        expect(nextPow2(0)).toBe(1);
        expect(nextPow2(1)).toBe(1);
    });
    it('returns next power for non-power inputs', () => {
        expect(nextPow2(3)).toBe(4);
        expect(nextPow2(5)).toBe(8);
        expect(nextPow2(1000)).toBe(1024);
    });
    it('returns same value for exact powers', () => {
        expect(nextPow2(2)).toBe(2);
        expect(nextPow2(16)).toBe(16);
        expect(nextPow2(1024)).toBe(1024);
    });
});

describe('InstanceBuffer.grow', () => {
    it('doubles capacity and preserves existing slot data', () => {
        const buf = new InstanceBuffer(2, [1]);
        buf.setBuffer(10, 0, new Float32Array([1, 2, 3, 4]));
        buf.setBuffer(20, 0, new Float32Array([5, 6, 7, 8]));
        const newTextures = buf.grow(4);
        expect(newTextures).toHaveLength(1);
        expect(buf.maxInstances).toBe(4);
        const slot10 = buf.keyToSlot.get(10)!;
        const slot20 = buf.keyToSlot.get(20)!;
        // textures[i].image.data is the same Float32Array as cpuData[i]
        const d = newTextures[0].image.data as Float32Array;
        expect(Array.from(d.slice(slot10 * 4, slot10 * 4 + 4))).toEqual([1, 2, 3, 4]);
        expect(Array.from(d.slice(slot20 * 4, slot20 * 4 + 4))).toEqual([5, 6, 7, 8]);
    });
    it('throws if newCapacity <= current maxInstances', () => {
        const buf = new InstanceBuffer(4, [1]);
        expect(() => buf.grow(4)).toThrow();
        expect(() => buf.grow(2)).toThrow();
    });
    it('marks all buffers dirty after grow', () => {
        const buf = new InstanceBuffer(2, [1]);
        buf.setBuffer(1, 0, new Float32Array(4));
        buf.compact();
        buf.clearDirty(0);
        buf.grow(4);
        expect(buf.isDirty(0)).toBe(true);
    });
    it('compact() returns true when DataTextures were grown', () => {
        const buf = new InstanceBuffer(2, [1]);
        buf.setBuffer(0, 0, val(0));
        buf.setBuffer(1, 0, val(1));
        expect(buf.compact()).toBe(false);
        buf.clearDirty(0);
        // overflow into CPU-only territory
        buf.setBuffer(2, 0, val(2));
        expect(buf.maxInstances).toBe(2); // GPU texture still old size
        expect(buf.compact()).toBe(true); // grew
        expect(buf.maxInstances).toBe(4); // GPU texture grown
        expectSlots(buf, [0, 1, 2]);
    });
});

describe('InstanceBuffer', () => {
    describe('set / remove basics', () => {
        it('setBuffer adds a key and compact updates length', () => {
            const buf = buf1(4);
            buf.setBuffer(1, 0, val(1));
            buf.compact();
            expect(buf.length).toBe(1);
            expectSlots(buf, [1]);
        });

        it('setBuffer same key twice updates data in place', () => {
            const buf = buf1(4);
            buf.setBuffer(1, 0, val(1));
            buf.setBuffer(1, 0, new Float32Array([99, 99, 99, 0]));
            expect(buf.length).toBe(1);
            buf.compact();
            const slot = buf.keyToSlot.get(1)!;
            expect(buf.toSlotArray(0, slot)[0]).toBe(99);
        });

        it('remove returns false for unknown key', () => {
            const buf = buf1(4);
            expect(buf.remove(42)).toBe(false);
        });

        it('setBuffer auto-grows CPU capacity when full', () => {
            const buf = buf1(2);
            buf.setBuffer(0, 0, val(0));
            buf.setBuffer(1, 0, val(1));
            // No longer returns false — CPU grows automatically
            expect(buf.setBuffer(2, 0, val(2))).toBe(true);
            expect(buf.length).toBe(3);
            // GPU texture is still the old size until compact()
            expect(buf.maxInstances).toBe(2);
            expect(buf.compact()).toBe(true); // grew
            expect(buf.maxInstances).toBe(4); // nextPow2(3) = 4
        });
    });

    describe('compact', () => {
        it('remove last element — compact trims tail', () => {
            const buf = buf1(4);
            buf.setBuffer(0, 0, val(0));
            buf.setBuffer(1, 0, val(1));
            buf.setBuffer(2, 0, val(2));
            buf.remove(2);
            buf.compact();
            expect(buf.length).toBe(2);
            expectSlots(buf, [0, 1]);
        });

        it('remove first element — compact fills gap from tail', () => {
            const buf = buf1(4);
            buf.setBuffer(0, 0, val(0));
            buf.setBuffer(1, 0, val(1));
            buf.setBuffer(2, 0, val(2));
            buf.remove(0);
            buf.compact();
            expect(buf.length).toBe(2);
            expectSlots(buf, [1, 2]);
        });

        it('remove middle element — compact fills gap', () => {
            const buf = buf1(4);
            buf.setBuffer(0, 0, val(0));
            buf.setBuffer(1, 0, val(1));
            buf.setBuffer(2, 0, val(2));
            buf.remove(1);
            buf.compact();
            expect(buf.length).toBe(2);
            expectSlots(buf, [0, 2]);
        });

        it('multiple gaps — compact leaves no holes in active range', () => {
            const buf = buf1(8);
            for (let i = 0; i < 8; i++) buf.setBuffer(i, 0, val(i));
            buf.remove(1);
            buf.remove(3);
            buf.remove(5);
            buf.remove(7);
            buf.compact();
            expect(buf.length).toBe(4);
            expectSlots(buf, [0, 2, 4, 6]);
        });

        it('remove all — compact returns false, length is 0', () => {
            const buf = buf1(4);
            buf.setBuffer(0, 0, val(0));
            buf.setBuffer(1, 0, val(1));
            buf.remove(0);
            buf.remove(1);
            expect(buf.compact()).toBe(false);
            expect(buf.length).toBe(0);
        });

        it('fill, remove all, re-fill — slots are reused correctly', () => {
            const buf = buf1(4);
            for (let i = 0; i < 4; i++) buf.setBuffer(i, 0, val(i));
            for (let i = 0; i < 4; i++) buf.remove(i);
            buf.compact();
            for (let i = 10; i < 14; i++) buf.setBuffer(i, 0, val(i));
            buf.compact();
            expect(buf.length).toBe(4);
            expectSlots(buf, [10, 11, 12, 13]);
        });

        it('multiple compacts in a row are idempotent', () => {
            const buf = buf1(4);
            buf.setBuffer(0, 0, val(0));
            buf.setBuffer(1, 0, val(1));
            buf.remove(0);
            buf.compact();
            buf.compact();
            buf.compact();
            expect(buf.length).toBe(1);
            expectSlots(buf, [1]);
        });

        it('remove-add cycle does not corrupt active data range', () => {
            const buf = buf1(4);
            buf.setBuffer(0, 0, val(0));
            buf.setBuffer(1, 0, val(1));
            buf.setBuffer(2, 0, val(2));
            buf.remove(1);
            buf.compact();
            buf.setBuffer(5, 0, val(5));
            buf.compact();
            expect(buf.length).toBe(3);
            expectSlots(buf, [0, 2, 5]);
        });

        it('gaps at both ends and in the middle', () => {
            const buf = buf1(6);
            for (let i = 0; i < 6; i++) buf.setBuffer(i, 0, val(i));
            buf.remove(0);
            buf.remove(2);
            buf.remove(5);
            buf.compact();
            expect(buf.length).toBe(3);
            expectSlots(buf, [1, 3, 4]);
        });
    });

    describe('dirty tracking', () => {
        it('setBuffer marks the target buffer dirty', () => {
            const buf = new InstanceBuffer(4, [1, 1]);
            buf.setBuffer(0, 1, new Float32Array([1, 2, 3, 4]));
            expect(buf.isDirty(0)).toBe(false);
            expect(buf.isDirty(1)).toBe(true);
        });

        it('clearDirty resets dirty flag', () => {
            const buf = buf1(4);
            buf.setBuffer(0, 0, val(0));
            expect(buf.isDirty(0)).toBe(true);
            buf.clearDirty(0);
            expect(buf.isDirty(0)).toBe(false);
        });

        it('compact after remove marks all buffers dirty', () => {
            const buf = new InstanceBuffer(4, [1, 1]);
            buf.setBuffer(0, 0, new Float32Array([1, 2, 3, 0]));
            buf.setBuffer(0, 1, new Float32Array([1, 2, 3, 4]));
            buf.clearDirty(0);
            buf.clearDirty(1);
            buf.remove(0);
            buf.compact();
            expect(buf.isDirty(0)).toBe(true);
            expect(buf.isDirty(1)).toBe(true);
        });

        it('compact without structural change does not mark buffers dirty', () => {
            const buf = buf1(4);
            buf.setBuffer(0, 0, val(0));
            buf.compact();
            buf.clearDirty(0);
            buf.compact();
            expect(buf.isDirty(0)).toBe(false);
        });
    });

    describe('multiple buffers', () => {
        it('independent buffers hold correct data', () => {
            const buf = new InstanceBuffer(4, [1, 1]);
            buf.setBuffer(0, 0, new Float32Array([1, 2, 3, 0]));
            buf.setBuffer(0, 1, new Float32Array([7, 0, 0, 0]));
            buf.compact();
            const slot = buf.keyToSlot.get(0)!;
            expect(buf.toSlotArray(0, slot)[0]).toBe(1);
            expect(buf.toSlotArray(1, slot)[0]).toBe(7);
        });

        it('moveSlot copies all buffers correctly', () => {
            const buf = new InstanceBuffer(4, [1, 1]);
            buf.setBuffer(0, 0, new Float32Array([10, 20, 30, 0]));
            buf.setBuffer(0, 1, new Float32Array([100, 0, 0, 0]));
            buf.setBuffer(1, 0, new Float32Array([11, 21, 31, 0]));
            buf.setBuffer(1, 1, new Float32Array([110, 0, 0, 0]));
            buf.remove(0); // force a slot move on compact
            buf.compact();
            const slot = buf.keyToSlot.get(1)!;
            expect(buf.toSlotArray(0, slot)[0]).toBe(11);
            expect(buf.toSlotArray(1, slot)[0]).toBe(110);
        });
    });
});
