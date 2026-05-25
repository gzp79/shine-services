import * as THREE from 'three';
import { Camera } from '../engine/camera/camera';
import { GameSystem } from '../engine/game-system';
import { RenderContext } from '../engine/render-context';
import type { Chunk } from '../world/chunk';
import type { ChunkCorner } from '../world/chunk-corner';
import type { ChunkEdge } from '../world/chunk-edge';
import { World } from '../world/world';

// Only switch to a new cell when its centroid is closer than this fraction
// of the current centroid's distance — creates a sticky hysteresis band.
const SWITCH_RADIUS_FACTOR = 0.6;

// Scratch state for the nearest-centroid scan — avoids per-frame allocations.
type BestOwner =
    | { type: 'cell'; owner: Chunk }
    | { type: 'edge-cell'; owner: ChunkEdge }
    | { type: 'corner-cell'; owner: ChunkCorner };

export class ChunkHoverSystem implements GameSystem {
    readonly name: string = 'Chunk Hover';

    // Reused scratch vectors — never escape this class.
    private readonly _worldPos = new THREE.Vector2();

    constructor(
        private readonly world: World,
        private readonly renderContext: RenderContext,
        private readonly camera: Camera
    ) {}

    update(_deltaTime: number): void {
        const mousePosition = this.renderContext.mousePosition;
        if (mousePosition.x === -1 && mousePosition.y === -1) {
            this.world.selection.clear();
            return;
        }

        const hit = this.camera.ndcToWorldPlanePoint(mousePosition.x, mousePosition.y);
        if (!hit) {
            this.world.selection.clear();
            return;
        }

        this._worldPos.set(hit.x, hit.y);
        const wpx = this._worldPos.x;
        const wpy = this._worldPos.y;
        const focusedChunkId = this.world.focusedChunkId;

        let bestDist = Infinity;
        let bestCellId = -1;
        let bestCx = 0;
        let bestCy = 0;
        let bestOwner: BestOwner | null = null;

        for (const chunk of this.world.chunks.values()) {
            if (chunk.id.distanceTo(focusedChunkId) > 1) continue;
            const groupPos = chunk.group.position;
            const centroids = chunk.centroids;
            const count = centroids.length / 2;
            for (let i = 0; i < count; i++) {
                const cx = centroids[i * 2] + groupPos.x;
                const cy = centroids[i * 2 + 1] + groupPos.y;
                const dist = (cx - wpx) * (cx - wpx) + (cy - wpy) * (cy - wpy);
                if (dist < bestDist) {
                    bestDist = dist;
                    bestCellId = i;
                    bestCx = cx;
                    bestCy = cy;
                    bestOwner = { type: 'cell', owner: chunk };
                }
            }
        }

        for (const edge of this.world.chunkEdges.values()) {
            if (edge.id.chunkId.distanceTo(focusedChunkId) > 1) continue;
            const groupPos = edge.group.position;
            const centroids = edge.centroids;
            const count = centroids.length / 2;
            for (let i = 0; i < count; i++) {
                const cx = centroids[i * 2] + groupPos.x;
                const cy = centroids[i * 2 + 1] + groupPos.y;
                const dist = (cx - wpx) * (cx - wpx) + (cy - wpy) * (cy - wpy);
                if (dist < bestDist) {
                    bestDist = dist;
                    bestCellId = i;
                    bestCx = cx;
                    bestCy = cy;
                    bestOwner = { type: 'edge-cell', owner: edge };
                }
            }
        }

        for (const corner of this.world.chunkCorners.values()) {
            if (corner.id.chunkId.distanceTo(focusedChunkId) > 1) continue;
            const groupPos = corner.group.position;
            const centroids = corner.centroids;
            const count = centroids.length / 2;
            for (let i = 0; i < count; i++) {
                const cx = centroids[i * 2] + groupPos.x;
                const cy = centroids[i * 2 + 1] + groupPos.y;
                const dist = (cx - wpx) * (cx - wpx) + (cy - wpy) * (cy - wpy);
                if (dist < bestDist) {
                    bestDist = dist;
                    bestCellId = i;
                    bestCx = cx;
                    bestCy = cy;
                    bestOwner = { type: 'corner-cell', owner: corner };
                }
            }
        }

        if (!bestOwner) {
            this.world.selection.clear();
            return;
        }

        const current = this.world.selection.current;
        if (current) {
            const cdx = current.centroid.x - wpx;
            const cdy = current.centroid.y - wpy;
            const currentDist = cdx * cdx + cdy * cdy;
            if (bestDist > currentDist * SWITCH_RADIUS_FACTOR * SWITCH_RADIUS_FACTOR) {
                return;
            }
        }

        // Allocate centroid Vector2 only for the winner, once per actual change.
        const centroid = new THREE.Vector2(bestCx, bestCy);
        switch (bestOwner.type) {
            case 'cell':
                this.world.selection.set({ type: 'cell', chunk: bestOwner.owner, cellId: bestCellId, centroid });
                break;
            case 'edge-cell':
                this.world.selection.set({ type: 'edge-cell', edge: bestOwner.owner, cellId: bestCellId, centroid });
                break;
            case 'corner-cell':
                this.world.selection.set({
                    type: 'corner-cell',
                    corner: bestOwner.owner,
                    cellId: bestCellId,
                    centroid
                });
                break;
        }
    }

    dispose(): void {}
}
