import { describe, expect, it } from 'vitest';
import { InstanceBuffer } from './instance-buffer';

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

describe('InstanceBuffer', () => {
    describe('set / remove basics', () => {
        it('setBuffer adds a key and compact returns count', () => {
            const buf = buf1(4);
            buf.setBuffer(1, 0, val(1));
            expect(buf.compact()).toBe(1);
            expectSlots(buf, [1]);
        });

        it('setBuffer same key twice updates data in place', () => {
            const buf = buf1(4);
            buf.setBuffer(1, 0, val(1));
            buf.setBuffer(1, 0, new Float32Array([99, 99, 99, 0]));
            expect(buf.length).toBe(1);
            expect(buf.compact()).toBe(1);
            const slot = buf.keyToSlot.get(1)!;
            expect(buf.toSlotArray(0, slot)[0]).toBe(99);
        });

        it('remove returns false for unknown key', () => {
            const buf = buf1(4);
            expect(buf.remove(42)).toBe(false);
        });

        it('setBuffer returns false when full', () => {
            const buf = buf1(2);
            buf.setBuffer(0, 0, val(0));
            buf.setBuffer(1, 0, val(1));
            expect(buf.setBuffer(2, 0, val(2))).toBe(false);
            expect(buf.length).toBe(2);
        });
    });

    describe('compact', () => {
        it('remove last element — compact trims tail', () => {
            const buf = buf1(4);
            buf.setBuffer(0, 0, val(0));
            buf.setBuffer(1, 0, val(1));
            buf.setBuffer(2, 0, val(2));
            buf.remove(2);
            expect(buf.compact()).toBe(2);
            expectSlots(buf, [0, 1]);
        });

        it('remove first element — compact fills gap from tail', () => {
            const buf = buf1(4);
            buf.setBuffer(0, 0, val(0));
            buf.setBuffer(1, 0, val(1));
            buf.setBuffer(2, 0, val(2));
            buf.remove(0);
            expect(buf.compact()).toBe(2);
            expectSlots(buf, [1, 2]);
        });

        it('remove middle element — compact fills gap', () => {
            const buf = buf1(4);
            buf.setBuffer(0, 0, val(0));
            buf.setBuffer(1, 0, val(1));
            buf.setBuffer(2, 0, val(2));
            buf.remove(1);
            expect(buf.compact()).toBe(2);
            expectSlots(buf, [0, 2]);
        });

        it('multiple gaps — compact leaves no holes in active range', () => {
            const buf = buf1(8);
            for (let i = 0; i < 8; i++) buf.setBuffer(i, 0, val(i));
            buf.remove(1);
            buf.remove(3);
            buf.remove(5);
            buf.remove(7);
            expect(buf.compact()).toBe(4);
            expectSlots(buf, [0, 2, 4, 6]);
        });

        it('remove all — compact returns 0', () => {
            const buf = buf1(4);
            buf.setBuffer(0, 0, val(0));
            buf.setBuffer(1, 0, val(1));
            buf.remove(0);
            buf.remove(1);
            expect(buf.compact()).toBe(0);
            expect(buf.length).toBe(0);
        });

        it('fill, remove all, re-fill — slots are reused correctly', () => {
            const buf = buf1(4);
            for (let i = 0; i < 4; i++) buf.setBuffer(i, 0, val(i));
            for (let i = 0; i < 4; i++) buf.remove(i);
            buf.compact();
            for (let i = 10; i < 14; i++) buf.setBuffer(i, 0, val(i));
            expect(buf.compact()).toBe(4);
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
            expect(buf.compact()).toBe(3);
            expectSlots(buf, [0, 2, 5]);
        });

        it('gaps at both ends and in the middle', () => {
            const buf = buf1(6);
            for (let i = 0; i < 6; i++) buf.setBuffer(i, 0, val(i));
            buf.remove(0);
            buf.remove(2);
            buf.remove(5);
            expect(buf.compact()).toBe(3);
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
