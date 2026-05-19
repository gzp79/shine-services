import { WasmWorld } from '#wasm';
import * as THREE from 'three';
import { PolygonData } from '../engine/mesh/geometry-data';
import { PolygonWireMesh } from '../engine/mesh/polygon-wire-mesh';
import { SelectionMesh } from '../engine/mesh/selection-mesh';
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
    private wireframe: PolygonWireMesh;
    private selection: SelectionMesh;

    constructor(
        private readonly world: WasmWorld,
        readonly id: ChunkCornerId,
        private readonly events: EventTarget
    ) {
        this.group.userData = { chunkCornerId: id, chunkCorner: this };
        const polygonData = this.buildPolygonData();
        this.selection = new SelectionMesh(this.group, polygonData);
        this.wireframe = new PolygonWireMesh(this.group, polygonData);
    }

    init(referenceChunkId: ChunkId): void {
        const offset = this.worldOffset(referenceChunkId);
        this.group.position.set(offset[0], offset[1], 0);
    }

    worldOffset(ref: ChunkId): Float32Array {
        return this.world.chunk_world_offset(ref.q, ref.r, this.id.chunkId.q, this.id.chunkId.r);
    }

    hideSelection(): void {
        this.selection.hide();
    }

    get showPolygonWire(): boolean {
        return this.wireframe.isVisible;
    }

    set showPolygonWire(value: boolean) {
        if (value) this.wireframe.show();
        else this.wireframe.hide();
    }

    dispose(): void {
        this.selection.dispose();
        this.wireframe.dispose();
    }

    private buildPolygonData(): PolygonData {
        const { q, r } = this.id.chunkId;
        const cornerIdx = this.id.cornerIdx;
        const vertices = this.world.boundary_corner_dual_polygon_vertices(q, r, cornerIdx);
        const packed = this.world.boundary_corner_dual_polygon(q, r, cornerIdx);

        if (packed.length === 0) {
            return new PolygonData(new Float32Array(), new Uint32Array(0), new Uint32Array(0));
        }

        const startsLen = packed[0];
        const starts = packed.slice(1, 1 + startsLen);
        const indices = packed.slice(1 + startsLen);
        return new PolygonData(vertices, indices, starts);
    }
}
