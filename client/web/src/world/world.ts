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
import { ChunkId } from './chunk-id';
import { chunkIdToWorldPosition } from './hex-utils';

export class World {
    private readonly SCOPE = 'World';
    readonly group = new THREE.Group();
    private readonly wasm: WasmWorld;
    private readonly chunks = new Map<string, Chunk>();
    private _referenceChunkId = ChunkId.ORIGIN;
    private _focusedChunkId = ChunkId.ORIGIN;
    private readonly subscriptions: EventSubscriptions;
    private readonly debugPanel: DebugPanel;
    private _showChunkLabels = false;
    private originCircle: THREE.Line;
    private chunk00Circle: THREE.Line;
    private pendingChunkUpdate: number | null = null;

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

    constructor(events: EventTarget, debugPanel: DebugPanel) {
        this.wasm = new WasmWorld();
        this.subscriptions = new EventSubscriptions(events);
        this.debugPanel = debugPanel;

        // Subscribe to world reference changed
        this.subscriptions.on<WorldReferenceChangedEvent>(WORLD_REFERENCE_CHANGED, this.handleWorldReferenceChanged);
        this.subscriptions.on<WorldCenterChangedEvent>(WORLD_CENTER_CHANGED, this.handleWorldCenterChanged);

        // Create debug circles
        this.originCircle = this.createDebugCircle(0xff0000, 0, 0); // Red at origin
        this.group.add(this.originCircle);

        const chunk00Pos = chunkIdToWorldPosition(this._referenceChunkId, ChunkId.ORIGIN);
        this.chunk00Circle = this.createDebugCircle(0x0000ff, chunk00Pos.x, chunk00Pos.y); // Blue at chunk(0,0)
        this.group.add(this.chunk00Circle);

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

        return chunk;
    }

    unloadChunk(id: ChunkId): void {
        const key = id.key();
        const chunk = this.chunks.get(key);
        if (!chunk) return;
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

        // Dispose chunks
        for (const chunk of this.chunks.values()) {
            this.group.remove(chunk.group);
            chunk.dispose();
        }
        this.chunks.clear();

        // Dispose debug circles
        this.group.remove(this.originCircle);
        this.originCircle.geometry.dispose();
        (this.originCircle.material as THREE.Material).dispose();

        this.group.remove(this.chunk00Circle);
        this.chunk00Circle.geometry.dispose();
        (this.chunk00Circle.material as THREE.Material).dispose();

        this.debugPanel.removeScope(this.SCOPE);
        this.wasm.free();
    }

    private handleWorldReferenceChanged = (event: WorldReferenceChangedEvent): void => {
        this._referenceChunkId = event.newChunkId;

        const delta = event.deltaPosition;
        for (const chunk of this.chunks.values()) {
            chunk.group.position.x += delta.x;
            chunk.group.position.y += delta.y;
        }

        // Move chunk(0,0) circle with chunks
        this.chunk00Circle.position.x += delta.x;
        this.chunk00Circle.position.y += delta.y;
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

    private createDebugCircle(color: number, x: number, y: number): THREE.Line {
        const radius = 100;
        const segments = 32;
        const geometry = new THREE.BufferGeometry();
        const positions = new Float32Array((segments + 1) * 3);

        for (let i = 0; i <= segments; i++) {
            const theta = (i / segments) * Math.PI * 2;
            positions[i * 3] = Math.cos(theta) * radius;
            positions[i * 3 + 1] = Math.sin(theta) * radius;
            positions[i * 3 + 2] = 0.1; // Slightly above ground
        }

        geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
        const material = new THREE.LineBasicMaterial({ color });
        const circle = new THREE.Line(geometry, material);
        circle.position.set(x, y, 0);
        return circle;
    }
}
