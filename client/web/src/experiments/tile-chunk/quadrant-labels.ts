import * as THREE from 'three';
import { TextSpriteFactory, type TextSpriteStyle } from '../../engine/resources/text-sprite';

const LABEL_Z = 5;
// Sprite size as a fraction of the center-to-corner distance, so text scales with the tile.
const LABEL_SCALE_FACTOR = 0.7;
const LABEL_STYLE: TextSpriteStyle = {
    font: 'bold 48px monospace',
    color: '#ffdd55',
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

/** Toggleable overlay: the per-quadrant u8 (`tile_vertices`) drawn midway between each tile's center and that quadrant's corner. */
export class QuadrantLabels {
    readonly group = new THREE.Group();
    private readonly parent: THREE.Object3D;
    private readonly sprites = new TextSpriteFactory();

    constructor(parent: THREE.Object3D, source: QuadrantLabelSource) {
        this.parent = parent;
        parent.add(this.group);
        this.build(source);
    }

    private build(source: QuadrantLabelSource): void {
        const centers = source.vertices();
        const corners = source.tile_distortions();
        const indices = source.indices();
        const quadrants = source.tile_vertices();
        if (!centers || !corners || !indices || !quadrants) return;

        for (let k = 0; k < indices.length; k++) {
            const tile = indices[k];
            const quadrant = quadrants[k];
            const cx = centers[tile * 2];
            const cy = centers[tile * 2 + 1];
            const vx = corners[tile * 8 + quadrant * 2];
            const vy = corners[tile * 8 + quadrant * 2 + 1];

            const sprite = this.sprites.create(String(quadrant), LABEL_STYLE);
            sprite.position.set((cx + vx) / 2, (cy + vy) / 2, LABEL_Z);
            const size = Math.hypot(vx - cx, vy - cy) * LABEL_SCALE_FACTOR;
            sprite.scale.set(size, size, 1);
            sprite.renderOrder = 999;
            this.group.add(sprite);
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
