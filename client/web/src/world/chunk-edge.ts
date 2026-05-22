import { EdgeCellsHandle, WasmWorld } from '#wasm';
import * as THREE from 'three';
import { PolygonWireMesh } from '../engine/mesh/polygon-wire-mesh';
import { SelectionMesh } from '../engine/mesh/selection-mesh';
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
    private wireframe: PolygonWireMesh;
    private selection: SelectionMesh;

    constructor(
        private readonly world: WasmWorld,
        readonly id: ChunkEdgeId,
        private readonly events: EventTarget
    ) {
        this.group.userData = { chunkEdgeId: id, chunkEdge: this };
        this.cells = world.edge_cells(id.chunkId.q, id.chunkId.r, id.edgeIdx)!;
        this.selection = new SelectionMesh(this.group, this.cells);
        this.wireframe = PolygonWireMesh.fromPolygons(this.group, this.cells);
    }

    init(referenceChunkId: ChunkId): void {
        const offset = this.worldOffset(referenceChunkId);
        this.group.position.set(offset[0], offset[1], 0);
    }

    worldOffset(ref: ChunkId): [number, number] {
        return this.world.chunk_world_offset(ref.q, ref.r, this.id.chunkId.q, this.id.chunkId.r);
    }

    hideSelection(): void {
        this.selection.hide();
    }

    get showCellWires(): boolean {
        return this.wireframe.isVisible();
    }

    set showCellWires(value: boolean) {
        if (value) this.wireframe.show();
        else this.wireframe.hide();
    }

    dispose(): void {
        this.selection.dispose();
        this.wireframe.dispose();
        this.cells.free();
    }
}
