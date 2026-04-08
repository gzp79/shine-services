import { WasmWorld } from '#wasm';
import * as THREE from 'three';
import { MAX_LOADED_CHUNK_DISTANCE, MAX_TRACKED_CHUNK_COUNT, MAX_TRACKED_CHUNK_DISTANCE } from '../engine/config';
import type { DebugPanel } from '../engine/debug-panel';
import { EventSubscriptions } from '../engine/events';
import {
    WORLD_CENTER_CHANGED,
    WORLD_REFERENCE_CHANGED,
    type WorldCenterChangedEvent,
    type WorldReferenceChangedEvent
} from '../systems/world-reference-system';
import { Chunk } from './chunk';
import { ChunkEdge, ChunkEdgeId } from './chunk-edge';
import { ChunkId } from './chunk-id';
import { worldPositionToChunkId } from './hex-utils';

type Selection =
    | { type: 'cell'; chunk: Chunk; cellId: number; worldPoint: THREE.Vector3; localPoint: THREE.Vector3 }
    | { type: 'edge-cell'; edge: ChunkEdge; cellId: number; worldPoint: THREE.Vector3; localPoint: THREE.Vector3 };

type WorldConsts = {
    chunkWorldSize: number;
    cellWorldSize: number;
};

export class World {
    private readonly SCOPE = 'World';
    readonly group = new THREE.Group();
    private readonly wasm: WasmWorld;
    private readonly chunks = new Map<string, Chunk>();
    private readonly chunkEdges = new Map<string, ChunkEdge>();
    private _referenceChunkId = ChunkId.ORIGIN;
    private _focusedChunkId = ChunkId.ORIGIN;
    private readonly subscriptions: EventSubscriptions;
    private readonly debugPanel: DebugPanel;
    private _showChunkLabels = false;
    private _showPolygonWire = false;
    private pendingChunkUpdate: number | null = null;
    private _hover: Selection | null = null;

    public readonly consts: WorldConsts;

    get referenceChunkId(): ChunkId {
        return this._referenceChunkId;
    }

    get focusedChunkId(): ChunkId {
        return this._focusedChunkId;
    }

    get showChunkLabels(): boolean {
        return this._showChunkLabels;
    }

    set showChunkLabels(value: boolean) {
        this._showChunkLabels = value;
        for (const chunk of this.chunks.values()) {
            chunk.showLabel = value;
        }
    }

    get showPolygonWire(): boolean {
        return this._showPolygonWire;
    }

    set showPolygonWire(value: boolean) {
        this._showPolygonWire = value;
        for (const chunk of this.chunks.values()) {
            chunk.showPolygonWire = value;
        }
        for (const edge of this.chunkEdges.values()) {
            edge.showPolygonWire = value;
        }
    }

    constructor(events: EventTarget, debugPanel: DebugPanel) {
        this.wasm = new WasmWorld();
        this.consts = {
            chunkWorldSize: this.wasm.const_chunk_world_size(),
            cellWorldSize: this.wasm.const_cell_world_size()
        };
        this.subscriptions = new EventSubscriptions(events);
        this.debugPanel = debugPanel;

        // Subscribe to world reference changed
        this.subscriptions.on<WorldReferenceChangedEvent>(WORLD_REFERENCE_CHANGED, this.handleWorldReferenceChanged);
        this.subscriptions.on<WorldCenterChangedEvent>(WORLD_CENTER_CHANGED, this.handleWorldCenterChanged);

        this.updateChunksAroundFocus();
    }

    loadChunk(id: ChunkId): Chunk {
        const key = id.key();
        const existing = this.chunks.get(key);
        if (existing) return existing;

        this.wasm.init_chunk(id.q, id.r);

        const chunk = new Chunk(this.wasm, id, this.subscriptions.events);
        this.group.add(chunk.group);
        this.chunks.set(key, chunk);
        this.updateDebugPanel();

        chunk.init(this._referenceChunkId);
        chunk.showLabel = this._showChunkLabels;
        chunk.showPolygonWire = this._showPolygonWire;

        this.updateChunkEdgesForChunk(id);

        return chunk;
    }

    unloadChunk(id: ChunkId): void {
        const key = id.key();
        const chunk = this.chunks.get(key);
        if (!chunk) return;

        if (this._hover?.type === 'cell' && this._hover.chunk === chunk) {
            this.clearHover();
        }

        this.removeChunkEdgesForChunk(id);

        this.group.remove(chunk.group);
        chunk.dispose();
        this.chunks.delete(key);
        this.wasm.remove_chunk(id.q, id.r);
        this.updateDebugPanel();
    }

    dispose(): void {
        // Cancel pending chunk update
        if (this.pendingChunkUpdate !== null) {
            cancelIdleCallback(this.pendingChunkUpdate);
            this.pendingChunkUpdate = null;
        }

        // Cleanup event listeners
        this.subscriptions.dispose();

        // Dispose boundary entities
        for (const edge of this.chunkEdges.values()) {
            this.group.remove(edge.group);
            edge.dispose();
        }
        this.chunkEdges.clear();

        // Dispose chunks
        for (const chunk of this.chunks.values()) {
            this.group.remove(chunk.group);
            chunk.dispose();
        }
        this.chunks.clear();

        this.debugPanel.removeScope(this.SCOPE);
        this.wasm.free();
    }

    private setHover(selection: Selection): void {
        // Hide previous selection
        if (this._hover) {
            switch (this._hover.type) {
                case 'cell':
                    this._hover.chunk.hideSelection();
                    break;
                case 'edge-cell':
                    this._hover.edge.hideSelection();
                    break;
            }
        }

        this._hover = selection;

        // Update debug panel
        switch (selection.type) {
            case 'cell':
                this.debugPanel.set(
                    this.SCOPE,
                    'Hover Cell',
                    `Chunk (${selection.chunk.id.q}, ${selection.chunk.id.r}), Cell ${selection.cellId}`
                );
                break;
            case 'edge-cell':
                this.debugPanel.set(
                    this.SCOPE,
                    'Hover Cell',
                    `Edge (${selection.edge.id.chunkId.q}, ${selection.edge.id.chunkId.r})-${selection.edge.id.edgeIdx}, Cell ${selection.cellId}`
                );
                break;
        }
    }

    setHoverAt(worldPos: THREE.Vector3) {
        if (this._hover) {
            const dist = worldPos.distanceTo(this._hover.worldPoint);
            if (dist < this.consts.cellWorldSize * 0.5) {
                return;
            }
        }

        const chunkId = worldPositionToChunkId(this._referenceChunkId, new THREE.Vector2(worldPos.x, worldPos.y));
        const chunk = this.chunks.get(chunkId.key());
        if (chunk) {
            const selection = chunk.showSelectionAt(worldPos);
            if (selection) {
                this.setHover({
                    type: 'cell',
                    chunk,
                    worldPoint: worldPos,
                    localPoint: selection.localPos,
                    cellId: selection.cellId
                });
                return;
            }
        }

        /*        // Try boundary edge entities
        for (const edge of this.chunkEdges.values()) {
            const selection = edge.showSelectionAt(worldPos);
            if (selection) {
                this.setHover({
                    type: 'edge-cell',
                    edge,
                    worldPoint: worldPos,
                    localPoint: selection.localPos,
                    cellId: selection.cellId
                });
                return;
            }
            }*/

        this.clearHover();
    }

    clearHover() {
        if (!this._hover) return;

        switch (this._hover.type) {
            case 'cell':
                this._hover.chunk.hideSelection();
                break;
            case 'edge-cell':
                this._hover.edge.hideSelection();
                break;
        }

        this._hover = null;
        this.debugPanel.set(this.SCOPE, 'Hover Cell', 'None');
    }

    private handleWorldReferenceChanged = (event: WorldReferenceChangedEvent): void => {
        this._referenceChunkId = event.newChunkId;

        const delta = event.deltaPosition;
        for (const chunk of this.chunks.values()) {
            chunk.group.position.x += delta.x;
            chunk.group.position.y += delta.y;
        }

        // Reposition boundary entities
        for (const edge of this.chunkEdges.values()) {
            edge.group.position.x += delta.x;
            edge.group.position.y += delta.y;
        }
    };

    private handleWorldCenterChanged = (event: WorldCenterChangedEvent): void => {
        this._focusedChunkId = event.newChunkId;

        // Cancel any pending chunk update
        if (this.pendingChunkUpdate !== null) {
            cancelIdleCallback(this.pendingChunkUpdate);
        }

        // Defer chunk loading to avoid blocking the current frame
        // This prevents stuttering when crossing chunk boundaries
        this.pendingChunkUpdate = requestIdleCallback(
            () => {
                this.pendingChunkUpdate = null;
                this.updateChunksAroundFocus();
            },
            { timeout: 16 } // Force execution within 16ms (1 frame at 60fps) if idle time isn't available
        );
    };

    private updateChunkEdgesForChunk(chunkId: ChunkId): void {
        for (const edgeIdx of [0, 1, 2] as const) {
            const edgeId = new ChunkEdgeId(chunkId, edgeIdx);
            const neighborId = edgeId.neighborChunkId();

            if (this.chunks.has(neighborId.key()) && !this.chunkEdges.has(edgeId.key())) {
                console.log(
                    `Creating boundary edge entity for edge ${edgeId.key()} between chunks ${chunkId.key()} and ${neighborId.key()}`
                );
                const entity = new ChunkEdge(this.wasm, edgeId, this.subscriptions.events);
                entity.init(this._referenceChunkId);
                entity.showPolygonWire = this._showPolygonWire;
                this.group.add(entity.group);
                this.chunkEdges.set(edgeId.key(), entity);
            }
        }
    }

    private removeChunkEdgesForChunk(chunkId: ChunkId): void {
        for (const [key, edge] of this.chunkEdges.entries()) {
            const neighborId = edge.id.neighborChunkId();
            if (edge.id.chunkId.equals(chunkId) || neighborId.equals(chunkId)) {
                // Clear hover if it references this entity
                if (this._hover?.type === 'edge-cell' && this._hover.edge === edge) {
                    this.clearHover();
                }

                this.group.remove(edge.group);
                edge.dispose();
                this.chunkEdges.delete(key);
            }
        }
    }

    private updateChunksAroundFocus(): void {
        // Load focused chunk and all neighbors
        for (const neighbor of this._focusedChunkId.spiral(MAX_LOADED_CHUNK_DISTANCE)) {
            this.loadChunk(neighbor);
        }

        // Unload chunks that are too far or if over limit
        const chunksWithDistance = Array.from(this.chunks.values())
            .map((chunk) => ({
                chunk,
                distance: chunk.id.distanceTo(this._focusedChunkId)
            }))
            .sort((a, b) => b.distance - a.distance); // Furthest first

        // Remove chunks beyond max distance
        const distantChunks = chunksWithDistance.filter((c) => c.distance > MAX_TRACKED_CHUNK_DISTANCE);
        for (const { chunk } of distantChunks) {
            this.unloadChunk(chunk.id);
        }

        // Remove furthest chunks if still over limit
        if (this.chunks.size > MAX_TRACKED_CHUNK_COUNT) {
            const chunksToRemove = this.chunks.size - MAX_TRACKED_CHUNK_COUNT;
            const remainingChunks = chunksWithDistance
                .filter((c) => c.distance <= MAX_TRACKED_CHUNK_DISTANCE)
                .slice(0, chunksToRemove);

            for (const { chunk } of remainingChunks) {
                this.unloadChunk(chunk.id);
            }
        }
    }

    private updateDebugPanel(): void {
        this.debugPanel.set(this.SCOPE, 'Loaded Chunks', this.chunks.size.toString());
    }
}
