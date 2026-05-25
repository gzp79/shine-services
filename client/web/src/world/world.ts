import { WasmWorld } from '#wasm';
import * as THREE from 'three';
import { MAX_LOADED_CHUNK_DISTANCE, MAX_TRACKED_CHUNK_COUNT, MAX_TRACKED_CHUNK_DISTANCE } from '../engine/config';
import type { DebugPanel } from '../engine/debug-panel';
import { EventSubscriptions } from '../engine/events';
import { span } from '../engine/utils';
import {
    WORLD_CENTER_CHANGED,
    WORLD_REFERENCE_CHANGED,
    type WorldCenterChangedEvent,
    type WorldReferenceChangedEvent
} from '../systems/world-reference-system';
import { Chunk } from './chunk';
import { ChunkCorner, ChunkCornerId } from './chunk-corner';
import { ChunkEdge, ChunkEdgeId } from './chunk-edge';
import { ChunkId, HexFlatDir, HexPointyDir } from './chunk-id';

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
    private readonly chunkCorners = new Map<string, ChunkCorner>();
    private _referenceChunkId = ChunkId.ORIGIN;
    private _focusedChunkId = ChunkId.ORIGIN;
    private readonly subscriptions: EventSubscriptions;
    private readonly debugPanel: DebugPanel;
    private _showChunkLabels = false;
    private _showCellWires = false;
    private pendingChunkUpdate: number | null = null;
    private loadQueue: ChunkId[] = [];
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

    get showCellWires(): boolean {
        return this._showCellWires;
    }

    set showCellWires(value: boolean) {
        this._showCellWires = value;
        for (const chunk of this.chunks.values()) {
            chunk.showCellWires = value;
        }
        for (const edge of this.chunkEdges.values()) {
            edge.showCellWires = value;
        }
        for (const corner of this.chunkCorners.values()) {
            corner.showCellWires = value;
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

        this.loadQueue = Array.from(this._focusedChunkId.spiral(MAX_LOADED_CHUNK_DISTANCE));
        this.scheduleLoadQueue();
    }

    loadChunk(id: ChunkId): Chunk {
        const key = id.key();
        const existing = this.chunks.get(key);
        if (existing) return existing;

        using _s = span(`loadChunk(${id.q},${id.r})`);

        this.wasm.init_chunk(id.q, id.r);

        const chunk = new Chunk(this.wasm, id, this.subscriptions.events);
        this.group.add(chunk.group);
        this.chunks.set(key, chunk);
        this.updateDebugPanel();

        chunk.init(this._referenceChunkId);
        chunk.showLabel = this._showChunkLabels;
        chunk.showCellWires = this._showCellWires;

        this.updateChunkEdgesForChunk(id);
        for (const dir of [HexFlatDir.SW, HexFlatDir.S, HexFlatDir.SE] as const) {
            this.updateChunkEdgesForChunk(id.neighbor(dir));
        }

        this.updateChunkCornersForChunk(id);
        for (const dir of [
            HexFlatDir.SW,
            HexFlatDir.S,
            HexFlatDir.SE,
            HexFlatDir.NE,
            HexFlatDir.N,
            HexFlatDir.NW
        ] as const) {
            this.updateChunkCornersForChunk(id.neighbor(dir));
        }

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
        this.removeChunkCornersForChunk(id);

        this.group.remove(chunk.group);
        chunk.dispose();
        this.chunks.delete(key);
        this.wasm.remove_chunk(id.q, id.r);
        this.updateDebugPanel();
    }

    dispose(): void {
        if (this.pendingChunkUpdate !== null) {
            cancelIdleCallback(this.pendingChunkUpdate);
            this.pendingChunkUpdate = null;
        }
        this.loadQueue = [];

        // Cleanup event listeners
        this.subscriptions.dispose();

        // Dispose boundary entities
        for (const edge of this.chunkEdges.values()) {
            this.group.remove(edge.group);
            edge.dispose();
        }
        this.chunkEdges.clear();

        for (const corner of this.chunkCorners.values()) {
            this.group.remove(corner.group);
            corner.dispose();
        }
        this.chunkCorners.clear();

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
                    // TODO: re-enable when Chunk.hideSelection is restored
                    break;
                case 'edge-cell':
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

        const chunkId = ChunkId.fromWorldPosition(this._referenceChunkId, new THREE.Vector2(worldPos.x, worldPos.y));
        const chunk = this.chunks.get(chunkId.key());
        if (chunk) {
            // TODO: re-enable when Chunk.showSelectionAt is restored
            // const selection = chunk.showSelectionAt(worldPos);
            // if (selection) {
            //     this.setHover({ type: 'cell', chunk, worldPoint: worldPos, localPoint: selection.localPos, cellId: selection.cellId });
            //     return;
            // }
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
                // TODO: re-enable when Chunk.hideSelection is restored
                break;
            case 'edge-cell':
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
        for (const edge of this.chunkEdges.values()) {
            edge.group.position.x += delta.x;
            edge.group.position.y += delta.y;
        }
        for (const corner of this.chunkCorners.values()) {
            corner.group.position.x += delta.x;
            corner.group.position.y += delta.y;
        }
    };

    private handleWorldCenterChanged = (event: WorldCenterChangedEvent): void => {
        this._focusedChunkId = event.newChunkId;

        if (this.pendingChunkUpdate !== null) {
            cancelIdleCallback(this.pendingChunkUpdate);
        }

        // Rebuild load queue sorted by distance (closest first), then drain it chunk-by-chunk
        this.loadQueue = Array.from(this._focusedChunkId.spiral(MAX_LOADED_CHUNK_DISTANCE)).filter(
            (id) => !this.chunks.has(id.key())
        );

        this.unloadDistantChunks();
        this.scheduleLoadQueue();
    };

    private scheduleLoadQueue(): void {
        if (this.loadQueue.length === 0) return;
        this.pendingChunkUpdate = requestIdleCallback((deadline) => {
            this.pendingChunkUpdate = null;
            while (this.loadQueue.length > 0 && deadline.timeRemaining() > 1) {
                this.loadChunk(this.loadQueue.shift()!);
            }
            this.scheduleLoadQueue();
        });
    }

    private updateChunkEdgesForChunk(chunkId: ChunkId): void {
        if (!this.chunks.has(chunkId.key())) {
            return;
        }

        for (const edgeIdx of [HexFlatDir.NE, HexFlatDir.N, HexFlatDir.NW] as const) {
            const edgeId = new ChunkEdgeId(chunkId, edgeIdx);
            if (this.chunkEdges.has(edgeId.key())) {
                continue;
            }
            const [, neighbor] = edgeId.involvedChunkIds();
            if (!this.chunks.has(neighbor.key())) {
                continue;
            }

            const entity = new ChunkEdge(this.wasm, edgeId, this.subscriptions.events);
            entity.init(this._referenceChunkId);
            entity.showCellWires = this._showCellWires;
            this.group.add(entity.group);
            this.chunkEdges.set(edgeId.key(), entity);
        }
    }

    private removeChunkEdgesForChunk(chunkId: ChunkId): void {
        for (const [key, edge] of this.chunkEdges.entries()) {
            if (edge.id.involvedChunkIds().some((id) => id.equals(chunkId))) {
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

    private updateChunkCornersForChunk(chunkId: ChunkId): void {
        if (!this.chunks.has(chunkId.key())) {
            return;
        }

        for (const cornerIdx of [HexPointyDir.E, HexPointyDir.NE, HexPointyDir.NW] as const) {
            const cornerId = new ChunkCornerId(chunkId, cornerIdx);
            if (this.chunkCorners.has(cornerId.key())) {
                continue;
            }
            const [, n1, n2] = cornerId.involvedChunkIds();
            if (!this.chunks.has(n1.key()) || !this.chunks.has(n2.key())) {
                continue;
            }

            const entity = new ChunkCorner(this.wasm, cornerId, this.subscriptions.events);
            entity.init(this._referenceChunkId);
            entity.showCellWires = this._showCellWires;
            this.group.add(entity.group);
            this.chunkCorners.set(cornerId.key(), entity);
        }
    }

    private removeChunkCornersForChunk(chunkId: ChunkId): void {
        for (const [key, corner] of this.chunkCorners.entries()) {
            if (corner.id.involvedChunkIds().some((id) => id.equals(chunkId))) {
                this.group.remove(corner.group);
                corner.dispose();
                this.chunkCorners.delete(key);
            }
        }
    }

    private unloadDistantChunks(): void {
        const chunksWithDistance = Array.from(this.chunks.values())
            .map((chunk) => ({ chunk, distance: chunk.id.distanceTo(this._focusedChunkId) }))
            .sort((a, b) => b.distance - a.distance);

        for (const { chunk } of chunksWithDistance.filter((c) => c.distance > MAX_TRACKED_CHUNK_DISTANCE)) {
            this.unloadChunk(chunk.id);
        }

        if (this.chunks.size > MAX_TRACKED_CHUNK_COUNT) {
            const chunksToRemove = this.chunks.size - MAX_TRACKED_CHUNK_COUNT;
            for (const { chunk } of chunksWithDistance
                .filter((c) => c.distance <= MAX_TRACKED_CHUNK_DISTANCE)
                .slice(0, chunksToRemove)) {
                this.unloadChunk(chunk.id);
            }
        }
    }

    private updateDebugPanel(): void {
        this.debugPanel.set(this.SCOPE, 'Loaded Chunks', this.chunks.size.toString());
    }
}
