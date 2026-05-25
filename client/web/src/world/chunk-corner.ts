import { CornerCellsHandle, WasmWorld } from '#wasm';
import * as THREE from 'three';
import { WireNode } from '../engine/nodes/wire-node';
import { ChunkId, HexFlatDir, HexPointyDir } from './chunk-id';

export class ChunkCornerId {
    constructor(
        public readonly chunkId: ChunkId,
        public readonly cornerIdx: HexPointyDir.E | HexPointyDir.NE | HexPointyDir.NW
    ) {}

    key(): string {
        return `${this.chunkId.key()}-c${this.cornerIdx}`;
    }

    equals(other: ChunkCornerId): boolean {
        return this.chunkId.equals(other.chunkId) && this.cornerIdx === other.cornerIdx;
    }

    involvedChunkIds(): [ChunkId, ChunkId, ChunkId] {
        const CORNER_NEIGHBORS: Record<HexPointyDir.E | HexPointyDir.NE | HexPointyDir.NW, [HexFlatDir, HexFlatDir]> = {
            [HexPointyDir.E]: [HexFlatDir.SE, HexFlatDir.NE],
            [HexPointyDir.NE]: [HexFlatDir.NE, HexFlatDir.N],
            [HexPointyDir.NW]: [HexFlatDir.N, HexFlatDir.NW]
        };
        const [n1, n2] = CORNER_NEIGHBORS[this.cornerIdx];
        return [this.chunkId, this.chunkId.neighbor(n1), this.chunkId.neighbor(n2)];
    }
}

export class ChunkCorner {
    readonly group = new THREE.Group();
    readonly cells: CornerCellsHandle;
    private wireframe: WireNode;

    constructor(
        private readonly world: WasmWorld,
        readonly id: ChunkCornerId,
        private readonly events: EventTarget
    ) {
        this.group.userData = { chunkCornerId: id, chunkCorner: this };
        this.cells = world.corner_cells(id.chunkId.q, id.chunkId.r, id.cornerIdx)!;
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
