import type { TileDistortionLike } from '../../../mesh/polygon-mesh';
import type { ModelSet } from '../../assets/model-set';

/**
 * Contract every tile-set asset (procedural or authored glTF) must follow to be usable as base-layer
 * tiles: one variant per name below, each baked at rotation 0 with its "up" corners matching the
 * name's digits read in row-major order [TL, TR, BL, BR] — e.g. 'q1000' has only its top-left corner raised.
 */
const TILE_VARIANT_NAMES = ['q0000', 'q1000', 'q1100', 'q1001', 'q1110', 'q1111'] as const;
export type TileVariantName = (typeof TILE_VARIANT_NAMES)[number];

export class TileInstanceResolver {
    private readonly variantByName = new Map<string, number>();

    constructor(modelSet: ModelSet) {
        modelSet.models.forEach((m, i) => this.variantByName.set(m.name, i));
    }

    /** Fill the `outDistortion` array for the given tile index and value and returns the variant index.
     * if no variant exists for this value undefined is returned and `outDistortion` is not modified. */
    resolveTrilinear(
        tiles: TileDistortionLike,
        tileIdx: number,
        value: number,
        height: number,
        output: Float32Array,
        offset: number
    ): number | undefined {
        const { name, cpOrder } = resolveTileValue(value);
        const variantIndex = this.variantByName.get(name);
        if (variantIndex === undefined) return undefined;

        const corners = tiles.distortions;
        const base = tileIdx * 8;
        for (let c = 0; c < 4; c++) {
            const x = corners[base + c * 2];
            const y = corners[base + c * 2 + 1];
            const cp = cpOrder[c];
            output[offset + cp * 3] = x;
            output[offset + cp * 3 + 1] = y;
            output[offset + cp * 3 + 2] = 0;
            output[offset + (cp + 4) * 3] = x;
            output[offset + (cp + 4) * 3 + 1] = y;
            output[offset + (cp + 4) * 3 + 2] = height;
        }

        return variantIndex;
    }

    /** Fill the `outDistortion` array for the given tile index and value and returns the variant index.
     * if no variant exists for this value undefined is returned and `outDistortion` is not modified.
     * Shared by every distortion method built on the TILE_Q9_SCHEMA buffer (corners + edge blends +
     * height), e.g. 'lagrangeQ9' and 'coonsBell'. */
    resolveQ9Schema(
        tiles: TileDistortionLike,
        tileIdx: number,
        value: number,
        height: number,
        output: Float32Array,
        offset: number
    ): number | undefined {
        const { name, cpOrder, edgeOrder } = resolveTileValue(value);
        const variantIndex = this.variantByName.get(name);
        if (variantIndex === undefined) return undefined;

        const corners = tiles.distortions;
        const edgeBlends = tiles.edgeBlends;
        const cornerBase = tileIdx * 8;
        const edgeBase = tileIdx * 4;
        for (let c = 0; c < 4; c++) {
            const cp = cpOrder[c];
            output[offset + cp * 2] = corners[cornerBase + c * 2];
            output[offset + cp * 2 + 1] = corners[cornerBase + c * 2 + 1];
        }
        for (let e = 0; e < 4; e++) {
            output[offset + 8 + edgeOrder[e]] = edgeBlends[edgeBase + e];
        }
        output[offset + 12] = height;

        return variantIndex;
    }
}

// Canonical (rotation-0) up/down pattern per name, in CCW real-corner order [BL, BR, TR, TL] — the
// same order the base-layer packs its per-quadrant bytes in and CCW_TO_CP indexes below.
const CANONICAL_PATTERN: Record<TileVariantName, readonly [number, number, number, number]> = {
    q0000: [0, 0, 0, 0],
    q1000: [0, 0, 0, 1],
    q1100: [0, 0, 1, 1],
    q1001: [0, 1, 0, 1],
    q1110: [1, 0, 1, 1],
    q1111: [1, 1, 1, 1]
};

// Real-corner CCW → cp-slot mapping at rotation 0 (see TileDistortion's cp layout in instanced-tile-set.ts):
// BL→cp0, BR→cp1, TR→cp3, TL→cp2 (cp2/cp3 swapped relative to CCW visit order).
const CCW_TO_CP: readonly [number, number, number, number] = [0, 1, 3, 2];

type TileValueEntry = {
    readonly name: TileVariantName;
    readonly cpOrder: readonly [number, number, number, number];
    // Raw edge e (connecting real corners e, e+1) -> canonical edge slot, same rotation as cpOrder but
    // no permutation: canonical edge slots are already defined in real-corner order [bottom, right, top,
    // left], i.e. edge slot k connects cp k and cp (k+1) in real-corner (not row-major) order.
    readonly edgeOrder: readonly [number, number, number, number];
};

// One entry per possible 4-bit up/down mask (bit c = real corner c is "up"), built by rotating each
// canonical pattern through all 4 quarter-turns.
const TILE_VALUE_TABLE: TileValueEntry[] = (() => {
    const table: TileValueEntry[] = new Array(16);
    for (const name of TILE_VARIANT_NAMES) {
        const canonical = CANONICAL_PATTERN[name];
        for (let r = 0; r < 4; r++) {
            let mask = 0;
            const cpOrder: [number, number, number, number] = [0, 0, 0, 0];
            const edgeOrder: [number, number, number, number] = [0, 0, 0, 0];
            for (let c = 0; c < 4; c++) {
                const src = (c - r + 4) % 4;
                mask |= canonical[src] << c;
                cpOrder[c] = CCW_TO_CP[src];
                edgeOrder[c] = src;
            }
            table[mask] = { name, cpOrder, edgeOrder };
        }
    }
    return table;
})();

/** Extracts the up/down bit for `corner` (0..3, CCW from BL) from a packed base-layer u32: currently
 *  0/1 per byte, so any nonzero byte counts as "up". */
function cornerUp(tileValue: number, corner: number): number {
    return (tileValue >>> (corner * 8)) & 0xff ? 1 : 0;
}

/** Resolves a tile's packed base-layer value to the variant name + cp rotation that renders it. */
function resolveTileValue(value: number): TileValueEntry {
    const mask = cornerUp(value, 0) | (cornerUp(value, 1) << 1) | (cornerUp(value, 2) << 2) | (cornerUp(value, 3) << 3);
    return TILE_VALUE_TABLE[mask]!;
}
