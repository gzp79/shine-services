import { ChunkId, HexFlatDir } from './chunk-id';

/**
 * Canonical identifier for boundary vertex entity.
 * Each vertex is owned by exactly one chunk (vertices 0, 1 only - top-left and top-right vertices of flat-top hex).
 */
export class BoundaryVertexId {
    constructor(
        public readonly chunkId: ChunkId,
        public readonly vertexIdx: 0 | 1
    ) {}

    /** Returns unique string key for this vertex (format: "q,r-vN" where N is 0-1). */
    key(): string {
        return `${this.chunkId.key()}-v${this.vertexIdx}`;
    }

    /** Check equality with another boundary vertex. */
    equals(other: BoundaryVertexId): boolean {
        return this.chunkId.equals(other.chunkId) && this.vertexIdx === other.vertexIdx;
    }

    /** Get the 2 other chunks meeting at this vertex */
    neighborChunkIds(): [ChunkId, ChunkId] {
        // Vertex 0 (top-left): neighbors at NW and N
        // Vertex 1 (top-right): neighbors at N and NE
        const [dir1, dir2] =
            this.vertexIdx === 0 ? ([HexFlatDir.NW, HexFlatDir.N] as const) : ([HexFlatDir.N, HexFlatDir.NE] as const);
        return [this.chunkId.neighbor(dir1), this.chunkId.neighbor(dir2)];
    }
}
