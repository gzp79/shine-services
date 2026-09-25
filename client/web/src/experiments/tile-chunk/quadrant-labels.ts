import * as THREE from 'three';
import { TextSpriteFactory, type TextSpriteStyle } from '../../engine/resources/text-sprite';

const LABEL_Z = 5;
// Sprite size as a fraction of the average center-to-corner distance across the chunk, so every
// label is the same size even where tile distortion makes individual quadrants uneven.
const LABEL_SCALE_FACTOR = 0.7;
const LABEL_STYLE: TextSpriteStyle = {
    font: 'bold 48px monospace',
    canvasWidth: 64,
    canvasHeight: 64
};

/** Buffers needed to place a label at every tile quadrant. */
export type QuadrantLabelSource = {
    /** Tile centers packed as [x, y, ...], one pair per tile. */
    vertices(): Float32Array | undefined;
    /** Tile quad corners packed as [x, y, ...], four pairs (octet) per tile. */
    tile_distortions(): Float32Array | undefined;
    /** Tile index of each polygon index entry. */
    indices(): Uint32Array | undefined;
    /** Quad-local vertex (0..4) of each polygon index entry. */
    tile_vertices(): Uint8Array | undefined;
};

/** Extracts the byte for `quadrant` (0..3) from a tile's packed base-layer u32 value (little-endian, matches Rust `to_le_bytes()`). */
function quadrantByte(tileValue: number, quadrant: number): number {
    return (tileValue >>> (quadrant * 8)) & 0xff;
}

/** Value 1 highlights the quadrant green; anything else (0 included) keeps the default label color. */
function quadrantColor(value: number): string {
    return value === 1 ? 'green' : '#ffdd55';
}

/**
 * Toggleable overlay: the per-quadrant base-layer byte, drawn midway between each tile's center
 * and that quadrant's corner. Text reflects the tile's synced base-layer value, not a fixed index.
 */
export class QuadrantLabels {
    readonly group = new THREE.Group();
    private readonly parent: THREE.Object3D;
    private readonly sprites = new TextSpriteFactory();
    private readonly entriesByTile = new Map<number, { sprite: THREE.Sprite; quadrant: number }[]>();

    constructor(parent: THREE.Object3D, source: QuadrantLabelSource, tileValues: Uint32Array) {
        this.parent = parent;
        parent.add(this.group);
        this.build(source, tileValues);
    }

    private build(source: QuadrantLabelSource, tileValues: Uint32Array): void {
        const centers = source.vertices();
        const corners = source.tile_distortions();
        const indices = source.indices();
        const quadrants = source.tile_vertices();
        if (!centers || !corners || !indices || !quadrants) return;
        if (indices.length === 0) return;

        let totalDistance = 0;
        for (let k = 0; k < indices.length; k++) {
            const tile = indices[k];
            const quadrant = quadrants[k];
            const cx = centers[tile * 2];
            const cy = centers[tile * 2 + 1];
            const vx = corners[tile * 8 + quadrant * 2];
            const vy = corners[tile * 8 + quadrant * 2 + 1];
            totalDistance += Math.hypot(vx - cx, vy - cy);
        }
        const size = (totalDistance / indices.length) * LABEL_SCALE_FACTOR;

        for (let k = 0; k < indices.length; k++) {
            const tile = indices[k];
            const quadrant = quadrants[k];
            const cx = centers[tile * 2];
            const cy = centers[tile * 2 + 1];
            const vx = corners[tile * 8 + quadrant * 2];
            const vy = corners[tile * 8 + quadrant * 2 + 1];

            const value = quadrantByte(tileValues[tile] ?? 0, quadrant);
            const sprite = this.sprites.create(String(value), { ...LABEL_STYLE, color: quadrantColor(value) });
            sprite.position.set((cx + vx) / 2, (cy + vy) / 2, LABEL_Z);
            sprite.scale.set(size, size, 1);
            sprite.renderOrder = 999;
            this.group.add(sprite);

            let entries = this.entriesByTile.get(tile);
            if (!entries) {
                entries = [];
                this.entriesByTile.set(tile, entries);
            }
            entries.push({ sprite, quadrant });
        }
    }

    /** Swaps the text of every quadrant sprite belonging to `tileIdx` to match its new base-layer value. Position/scale untouched. */
    updateTileValue(tileIdx: number, tileValue: number): void {
        const entries = this.entriesByTile.get(tileIdx);
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
