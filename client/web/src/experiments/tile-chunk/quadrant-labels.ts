import * as THREE from 'three';
import { TextSpriteFactory, type TextSpriteStyle } from '../../engine/resources/text-sprite';

const LABEL_Z = 5;
// Sprite size as a fraction of the average centroid-to-corner distance across the chunk, so every
// label is the same size even where tile distortion makes individual quadrants uneven.
const LABEL_SCALE_FACTOR = 0.7;
const LABEL_STYLE: TextSpriteStyle = {
    font: 'bold 48px monospace',
    canvasWidth: 64,
    canvasHeight: 64
};

/** Buffers needed to place a label at every tile quadrant. */
export type QuadrantLabelSource = {
    /** Tile quad corners packed as [x, y, ...], four pairs (octet) per tile, CCW from BL. */
    tile_distortions(): Float32Array | undefined;
};

/** Extracts the byte for `quadrant` (0..3) from a tile's packed base-layer u32 value (little-endian, matches Rust `to_le_bytes()`). */
function quadrantByte(tileValue: number, quadrant: number): number {
    return (tileValue >>> (quadrant * 8)) & 0xff;
}

/** Value 1 highlights the quadrant green; anything else (0 included) keeps the default label color. */
function quadrantColor(value: number): string {
    return value === 1 ? 'green' : '#ffdd55';
}

/** Average of a tile's 4 distortion corners — the label anchor, not any stored tile-center point. */
function tileCentroid(corners: Float32Array, tileIdx: number): { cx: number; cy: number } {
    const base = tileIdx * 8;
    let cx = 0;
    let cy = 0;
    for (let q = 0; q < 4; q++) {
        cx += corners[base + q * 2];
        cy += corners[base + q * 2 + 1];
    }
    return { cx: cx / 4, cy: cy / 4 };
}

/**
 * Toggleable overlay: the per-quadrant base-layer byte, drawn midway between each tile's distortion
 * centroid and that quadrant's corner. One tile in `tile_distortions()` order per array position, all
 * 4 quadrants always labeled — text reflects the tile's synced base-layer value, not a fixed index.
 */
export class QuadrantLabels {
    readonly group = new THREE.Group();
    private readonly parent: THREE.Object3D;
    private readonly sprites = new TextSpriteFactory();
    private readonly entriesByTile: { sprite: THREE.Sprite; quadrant: number }[][] = [];

    constructor(parent: THREE.Object3D, source: QuadrantLabelSource, tileValues: Uint32Array) {
        this.parent = parent;
        parent.add(this.group);
        this.build(source, tileValues);
    }

    private build(source: QuadrantLabelSource, tileValues: Uint32Array): void {
        const corners = source.tile_distortions();
        if (!corners) return;
        const tileCount = corners.length / 8;
        if (tileCount === 0) return;

        let totalDistance = 0;
        for (let t = 0; t < tileCount; t++) {
            const { cx, cy } = tileCentroid(corners, t);
            for (let q = 0; q < 4; q++) {
                const vx = corners[t * 8 + q * 2];
                const vy = corners[t * 8 + q * 2 + 1];
                totalDistance += Math.hypot(vx - cx, vy - cy);
            }
        }
        const size = (totalDistance / (tileCount * 4)) * LABEL_SCALE_FACTOR;

        for (let t = 0; t < tileCount; t++) {
            const { cx, cy } = tileCentroid(corners, t);
            const entries: { sprite: THREE.Sprite; quadrant: number }[] = [];

            for (let q = 0; q < 4; q++) {
                const vx = corners[t * 8 + q * 2];
                const vy = corners[t * 8 + q * 2 + 1];

                const value = quadrantByte(tileValues[t] ?? 0, q);
                const sprite = this.sprites.create(String(value), { ...LABEL_STYLE, color: quadrantColor(value) });
                sprite.position.set((cx + vx) / 2, (cy + vy) / 2, LABEL_Z);
                sprite.scale.set(size, size, 1);
                sprite.renderOrder = 999;
                this.group.add(sprite);

                entries.push({ sprite, quadrant: q });
            }
            this.entriesByTile.push(entries);
        }
    }

    /** Swaps the text of every quadrant sprite belonging to `tileIdx` to match its new base-layer value. Position/scale untouched. */
    updateTileValue(tileIdx: number, tileValue: number): void {
        const entries = this.entriesByTile[tileIdx];
        if (!entries) return;
        for (const { sprite, quadrant } of entries) {
            const value = quadrantByte(tileValue, quadrant);
            sprite.material = this.sprites.create(String(value), {
                ...LABEL_STYLE,
                color: quadrantColor(value)
            }).material;
        }
    }

    show(): void {
        this.group.visible = true;
    }

    hide(): void {
        this.group.visible = false;
    }

    dispose(): void {
        this.parent.remove(this.group);
        this.group.clear();
        this.sprites.dispose();
    }
}
