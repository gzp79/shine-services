import * as THREE from 'three';
import type { Camera } from '../camera/camera';
import type { DebugPanel } from '../engine/debug-panel';
import { EventDispatcher } from '../engine/events';
import type { GameSystem } from '../engine/game-system';
import { ChunkId } from '../world/chunk-id';
import { chunkIdToWorldPosition, worldPositionToChunkId } from '../world/hex-utils';
import type { World } from '../world/world';

const CHUNK_WORLD_SIZE = 1000;
/// Approximatelly 5 chunk away from the world reference
const REPOSITION_THRESHOLD = 25 * CHUNK_WORLD_SIZE * CHUNK_WORLD_SIZE;

// WorldReferenceChangedEvent - The camera moved away too much from the world center, it's time to rebase world values
export const WORLD_REFERENCE_CHANGED = 'worldreferencechanged';
export type WorldReferenceChangedEvent = {
    oldChunkId: ChunkId;
    newChunkId: ChunkId;
    deltaPosition: THREE.Vector2;
};

// WorldCenterChangedEvent - The cell in focus has changed
export const WORLD_CENTER_CHANGED = 'worldcenterchanged';
export type WorldCenterChangedEvent = {
    oldChunkId: ChunkId;
    newChunkId: ChunkId;
};

export class WorldReferenceSystem implements GameSystem {
    private readonly SCOPE = 'World Reference';
    private readonly dispatcher: EventDispatcher;

    constructor(
        private readonly camera: Camera,
        private readonly world: World,
        events: EventTarget,
        private readonly debugPanel: DebugPanel
    ) {
        this.dispatcher = new EventDispatcher(events);
    }

    update(_deltaTime: number): void {
        const worldPos = this.camera.worldPosition;
        const referenceChunkId = this.world.referenceChunkId;
        const focusedChunkId = this.world.focusedChunkId;

        const currentChunkId = worldPositionToChunkId(referenceChunkId, worldPos);
        const currentCenter = chunkIdToWorldPosition(referenceChunkId, currentChunkId);
        const distanceSq = currentCenter.lengthSq();

        // Update debug panel
        this.debugPanel.set(this.SCOPE, 'Camera Pos', `(${worldPos.x.toFixed(0)}, ${worldPos.y.toFixed(0)})`);
        this.debugPanel.set(this.SCOPE, 'Current Chunk', `(${currentChunkId.q}, ${currentChunkId.r})`);
        this.debugPanel.set(this.SCOPE, 'Reference Chunk', `(${referenceChunkId.q}, ${referenceChunkId.r})`);
        this.debugPanel.set(this.SCOPE, 'Focused Chunk', `(${focusedChunkId.q}, ${focusedChunkId.r})`);
        this.debugPanel.set(this.SCOPE, 'Distance', Math.sqrt(distanceSq).toFixed(0));

        // Check if focused chunk changed
        if (focusedChunkId.q !== currentChunkId.q || focusedChunkId.r !== currentChunkId.r) {
            // Check if reposition needed
            if (distanceSq > REPOSITION_THRESHOLD) {
                this.dispatcher.dispatch<WorldReferenceChangedEvent>(WORLD_REFERENCE_CHANGED, {
                    oldChunkId: referenceChunkId,
                    newChunkId: currentChunkId,
                    deltaPosition: currentCenter.negate()
                });
            }

            // Dispatch focus change
            this.dispatcher.dispatch<WorldCenterChangedEvent>(WORLD_CENTER_CHANGED, {
                oldChunkId: focusedChunkId,
                newChunkId: currentChunkId
            });
        }
    }

    destroy(): void {
        this.debugPanel.removeScope(this.SCOPE);
    }
}
