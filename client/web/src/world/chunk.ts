import { WasmWorld } from '#wasm';
import * as THREE from 'three';
import { MeshBuilder, buildMesh } from './mesh-builder';
import { ChunkId } from './types';

export class Chunk {
    readonly group = new THREE.Group();
    private mesh: MeshBuilder | null = null;

    constructor(
        private readonly world: WasmWorld,
        readonly id: ChunkId
    ) {
        this.group.userData = { chunkId: { q: id.q, r: id.r } };
    }

    buildMesh(reference: ChunkId): void {
        this.disposeMesh();

        const color = new THREE.Color().setHSL(this.chunkHash() / 0xffffffff, 0.5, 0.6);
        this.mesh = buildMesh(this, color);
        this.group.add(this.mesh.group);

        const offset = this.worldOffset(reference);
        this.group.position.set(offset[0], offset[1], 0);
    }

    vertices(): Float32Array {
        return this.world.chunk_vertices(this.id.q, this.id.r);
    }

    quadIndices(): Uint32Array {
        return this.world.chunk_quad_indices(this.id.q, this.id.r);
    }

    borderIndices(): Uint32Array {
        return this.world.chunk_border_indices(this.id.q, this.id.r);
    }

    worldOffset(ref: ChunkId): Float32Array {
        return this.world.chunk_world_offset(ref.q, ref.r, this.id.q, this.id.r);
    }

    /** Simple hash for deterministic debug color. */
    private chunkHash(): number {
        let h = ((this.id.q | 0) * 0x9e3779b9 + (this.id.r | 0)) >>> 0;
        h ^= h >>> 16;
        h = Math.imul(h, 0x45d9f3b) >>> 0;
        h ^= h >>> 16;
        return h;
    }

    disposeMesh(): void {
        if (this.mesh) {
            this.group.remove(this.mesh.group);
            this.mesh.dispose();
            this.mesh = null;
        }
    }
}
