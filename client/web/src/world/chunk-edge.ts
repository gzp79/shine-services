import { EdgeCellsHandle, WasmWorld } from '#wasm';
import * as THREE from 'three';
import { WireNode } from '../engine/nodes/wire-node';
import { ChunkId, HexFlatDir } from './chunk-id';

export class ChunkEdgeId {
    constructor(
        // The "owner" chunk id. Edge data is stored relative to this chunk.
        public readonly chunkId: ChunkId,
        public readonly edgeIdx: HexFlatDir.NE | HexFlatDir.N | HexFlatDir.NW
    ) {}

    key(): string {
        return `${this.chunkId.key()}-e${this.edgeIdx}`;
    }

    equals(other: ChunkEdgeId): boolean {
        return this.chunkId.equals(other.chunkId) && this.edgeIdx === other.edgeIdx;
    }

    involvedChunkIds(): [ChunkId, ChunkId] {
        return [this.chunkId, this.chunkId.neighbor(this.edgeIdx)];
    }
}

export class ChunkEdge {
    readonly group = new THREE.Group();
    readonly cells: EdgeCellsHandle;
    private wireframe: WireNode;

    constructor(
        private readonly world: WasmWorld,
        readonly id: ChunkEdgeId,
        private readonly events: EventTarget
    ) {
        this.group.userData = { chunkEdgeId: id, chunkEdge: this };
        this.cells = world.edge_cells(id.chunkId.q, id.chunkId.r, id.edgeIdx)!;
        this.wireframe = WireNode.fromPolygons(this.group, this.cells);
    }

    init(referenceChunkId: ChunkId): void {
        const offset = this.worldOffset(referenceChunkId);
        this.group.position.set(offset[0], offset[1], 0);
    }

    worldOffset(ref: ChunkId): [number, number] {
        return this.world.chunk_world_offset(ref.q, ref.r, this.id.chunkId.q, this.id.chunkId.r);
    }

    get showCellWires(): boolean {
        return this.wireframe.isVisible();
    }

    set showCellWires(value: boolean) {
        if (value) this.wireframe.show();
        else this.wireframe.hide();
    }

    dispose(): void {
        this.wireframe.dispose();
        this.cells.free();
    }
}
