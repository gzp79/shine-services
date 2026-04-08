import * as THREE from 'three';
import type { WorldCursor } from '../avatar/world-cursor';
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
    readonly name: string = 'World Reference';

    private readonly dispatcher: EventDispatcher;

    constructor(
        private readonly worldCursor: WorldCursor,
        private readonly world: World,
        events: EventTarget,
        private readonly debugPanel: DebugPanel
    ) {
        this.dispatcher = new EventDispatcher(events);
    }

    update(_deltaTime: number): void {
        const avatarPos = new THREE.Vector2(this.worldCursor.position.x, this.worldCursor.position.y);
        const referenceChunkId = this.world.referenceChunkId;
        const focusedChunkId = this.world.focusedChunkId;

        const currentChunkId = worldPositionToChunkId(referenceChunkId, avatarPos);
        const currentCenter = chunkIdToWorldPosition(referenceChunkId, currentChunkId);
        const distanceSq = currentCenter.lengthSq();

        // Update debug panel
        this.debugPanel.set(this.name, 'Avatar Pos', `(${avatarPos.x.toFixed(0)}, ${avatarPos.y.toFixed(0)})`);
        this.debugPanel.set(this.name, 'Current Chunk', `(${currentChunkId.q}, ${currentChunkId.r})`);
        this.debugPanel.set(this.name, 'Reference Chunk', `(${referenceChunkId.q}, ${referenceChunkId.r})`);
        this.debugPanel.set(this.name, 'Focused Chunk', `(${focusedChunkId.q}, ${focusedChunkId.r})`);
        this.debugPanel.set(this.name, 'Distance', Math.sqrt(distanceSq).toFixed(0));

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

    dispose(): void {
        this.debugPanel.removeScope(this.name);
    }
}
