import * as THREE from 'three';
import type { ICamera } from '../engine/camera/camera';
import type { DebugPanel } from '../engine/debug-panel';
import { GameSystem } from '../engine/game-system';
import type { IInputState } from '../engine/input/input-state';
import { CHUNK_WORLD_SIZE, MAX_ACTIVE_CHUNK_DISTANCE } from '../constants';
import { ChunkId } from '../world/chunk-id';
import { Selection } from '../world/selection/selection-event';
import { World } from '../world/world';

// Only switch to a new cell when its centroid is closer than this fraction
// of the current centroid's distance — creates a sticky hysteresis band.
const SWITCH_RADIUS_FACTOR = 0.6;

// Radius used to clamp the cursor to the interactable zone boundary.
const INTERACTION_RADIUS = CHUNK_WORLD_SIZE * (MAX_ACTIVE_CHUNK_DISTANCE + 1);

const SCOPE = 'Selection';

export class SelectionSystem implements GameSystem {
    readonly name: string = 'Selection';

    // Reused scratch vectors — never escape this class.
    private readonly _worldPos = new THREE.Vector2();
    private readonly _queryPos = new THREE.Vector2();
    private readonly debugPanel: DebugPanel;

    constructor(
        private readonly world: World,
        private readonly input: IInputState,
        private readonly camera: ICamera,
        debugPanel: DebugPanel
    ) {
        this.debugPanel = debugPanel;
    }

    update(_deltaTime: number): void {
        const mousePos = this.input.pointerPos;
        if (!mousePos) {
            this.debugPanel.set(SCOPE, 'Hit', 'None');
            this.world.selection.clear();
            return;
        }

        const hit = this.camera.screenToWorldPlanePoint(mousePos.x, mousePos.y);
        if (!hit) {
            this.debugPanel.set(SCOPE, 'Hit', 'None');
            this.world.selection.clear();
            return;
        }

        this._worldPos.set(hit.x, hit.y);
        const wpx = this._worldPos.x;
        const wpy = this._worldPos.y;
        this.debugPanel.set(SCOPE, 'Hit', `(${wpx.toFixed(2)}, ${wpy.toFixed(2)})`);

        const focusedChunkId = this.world.focusedChunkId;

        // Clamp query point to the interaction radius around the focused chunk center.
        const focusCenter = focusedChunkId.toWorldPosition(this.world.referenceChunkId);
        this._queryPos.set(wpx - focusCenter.x, wpy - focusCenter.y);
        if (this._queryPos.lengthSq() > INTERACTION_RADIUS * INTERACTION_RADIUS) {
            this._queryPos.normalize().multiplyScalar(INTERACTION_RADIUS);
        }
        const qpx = this._queryPos.x + focusCenter.x;
        const qpy = this._queryPos.y + focusCenter.y;

        let bestDist = Infinity;
        let best: Selection | null = null;

        for (const chunk of this.world.chunks.values()) {
            if (!chunk.id.isInteractable(focusedChunkId)) continue;
            const groupPos = chunk.group.position;
            const centroids = chunk.centroids;
            const count = centroids.length / 2;
            for (let i = 0; i < count; i++) {
                const cx = centroids[i * 2] + groupPos.x;
                const cy = centroids[i * 2 + 1] + groupPos.y;
                const dist = (cx - qpx) * (cx - qpx) + (cy - qpy) * (cy - qpy);
                if (dist < bestDist) {
                    bestDist = dist;
                    best = { type: 'cell', chunk, cellId: i };
                }
            }
        }

        for (const edge of this.world.chunkEdges.values()) {
            if (!edge.id.isInteractable(focusedChunkId)) continue;
            const groupPos = edge.group.position;
            const centroids = edge.centroids;
            const count = centroids.length / 2;
            for (let i = 0; i < count; i++) {
                const cx = centroids[i * 2] + groupPos.x;
                const cy = centroids[i * 2 + 1] + groupPos.y;
                const dist = (cx - qpx) * (cx - qpx) + (cy - qpy) * (cy - qpy);
                if (dist < bestDist) {
                    bestDist = dist;
                    best = { type: 'edge-cell', edge, cellId: i };
                }
            }
        }

        for (const corner of this.world.chunkCorners.values()) {
            if (!corner.id.isInteractable(focusedChunkId)) continue;
            const groupPos = corner.group.position;
            const centroids = corner.centroids;
            const count = centroids.length / 2;
            for (let i = 0; i < count; i++) {
                const cx = centroids[i * 2] + groupPos.x;
                const cy = centroids[i * 2 + 1] + groupPos.y;
                const dist = (cx - qpx) * (cx - qpx) + (cy - qpy) * (cy - qpy);
                if (dist < bestDist) {
                    bestDist = dist;
                    best = { type: 'corner-cell', corner, cellId: i };
                }
            }
        }

        if (!best) {
            this.world.selection.clear();
            return;
        }

        const cursorChunkId = ChunkId.fromWorldPosition(this.world.referenceChunkId, this._worldPos);
        const current = this.world.selection.current;
        if (
            current &&
            Selection.isInteractable(current, focusedChunkId) &&
            cursorChunkId.isInteractable(focusedChunkId)
        ) {
            const owner = Selection.owner(current);
            const ownerPos = owner.group.position;
            const ccx = owner.centroids[current.cellId * 2] + ownerPos.x;
            const ccy = owner.centroids[current.cellId * 2 + 1] + ownerPos.y;
            const currentDist = (ccx - qpx) * (ccx - qpx) + (ccy - qpy) * (ccy - qpy);
            if (bestDist > currentDist * SWITCH_RADIUS_FACTOR * SWITCH_RADIUS_FACTOR) {
                return;
            }
        }

        this.world.selection.set(best);
    }

    dispose(): void {
        this.debugPanel.removeScope(SCOPE);
    }
}
