/** Structural shape of bit-set data: one bit per index, packed into 32-bit words. */
export type BitSetLike = {
    readonly length: number;
    readonly packedWords: Uint32Array;
};

/** A WASM-side source of the internal packed bit-set representation. */
export type BitSetSource = {
    len(): number;
    packed_words(): Uint32Array;
};

/** Adapts a `BitSetSource` (e.g. `ChangeLog`) to a `BitSetLike`. */
export function asBitSet(source: BitSetSource): BitSetLike {
    return {
        get length() {
            return source.len();
        },
        get packedWords() {
            return source.packed_words();
        }
    };
}

/** Read-only JavaScript access to bit-set data wrapped in a convenient interface. */
export class ReadonlyBitSet implements Iterable<number> {
    readonly length: number;
    readonly words: Readonly<Uint32Array>;

    constructor(bits: BitSetLike) {
        this.length = bits.length;
        this.words = bits.packedWords;
    }

    has(index: number): boolean {
        if (!Number.isInteger(index) || index < 0 || index >= this.length) return false;
        return (this.words[index >>> 5]! & (1 << (index & 31))) !== 0;
    }

    /** Fast path for applying work to every set index without iterator allocations. */
    forEachSet(callback: (index: number) => void): void {
        for (let wordIndex = 0; wordIndex < this.words.length; wordIndex++) {
            let word = this.words[wordIndex]!;
            const base = wordIndex * 32;
            while (word !== 0) {
                const bit = 31 - Math.clz32(word & -word);
                callback(base + bit);
                word = (word & (word - 1)) >>> 0;
            }
        }
    }

    *[Symbol.iterator](): IterableIterator<number> {
        for (let wordIndex = 0; wordIndex < this.words.length; wordIndex++) {
            let word = this.words[wordIndex]!;
            const base = wordIndex * 32;
            while (word !== 0) {
                const bit = 31 - Math.clz32(word & -word);
                yield base + bit;
                word = (word & (word - 1)) >>> 0;
            }
        }
    }
}
