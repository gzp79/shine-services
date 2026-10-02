import { describe, expect, it } from 'vitest';
import { type BitSetSource, ReadonlyBitSet, asBitSet } from './bit-set';

describe('ReadonlyBitSet', () => {
    it('scans set indices locally across word boundaries', () => {
        const source: BitSetSource = {
            len: () => 65,
            packed_words: () => new Uint32Array([1, 0x80000001, 1])
        };

        const bits = new ReadonlyBitSet(asBitSet(source));
        const visited: number[] = [];
        bits.forEachSet((index) => visited.push(index));

        expect(visited).toEqual([0, 32, 63, 64]);
        expect([...bits]).toEqual(visited);
        expect(bits.has(63)).toBe(true);
        expect(bits.has(62)).toBe(false);
        expect(bits.has(65)).toBe(false);
    });
});
