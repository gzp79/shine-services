import { WasmWorld } from '#wasm';
import * as THREE from 'three';
import { Chunk } from './chunk';
import { ChunkId } from './types';

export class World {
    readonly group = new THREE.Group();
    private readonly wasm: WasmWorld;
    private readonly chunks = new Map<string, Chunk>();
    private _referenceCenter = ChunkId.ORIGIN;

    get referenceCenter(): ChunkId {
        return this._referenceCenter;
    }

    constructor() {
        this.wasm = new WasmWorld();
    }

    loadChunk(id: ChunkId): Chunk {
        const key = id.key();
        const existing = this.chunks.get(key);
        if (existing) return existing;

        this.wasm.init_chunk(id.q, id.r);

        const chunk = new Chunk(this.wasm, id);
        chunk.buildMesh(this._referenceCenter);
        this.group.add(chunk.group);
        this.chunks.set(key, chunk);
        return chunk;
    }

    unloadChunk(id: ChunkId): void {
        const key = id.key();
        const chunk = this.chunks.get(key);
        if (!chunk) return;
        this.group.remove(chunk.group);
        chunk.disposeMesh();
        this.chunks.delete(key);
        this.wasm.remove_chunk(id.q, id.r);
    }

    dispose(): void {
        for (const chunk of this.chunks.values()) {
            this.group.remove(chunk.group);
            chunk.disposeMesh();
        }
        this.chunks.clear();
        this.wasm.free();
    }
}
