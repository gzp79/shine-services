import { MAX_ACTIVE_CHUNK_DISTANCE, MAX_TRACKED_CHUNK_DISTANCE } from '../engine/config';
import { range } from '../engine/utils';

/** Hex direction offsets in axial coordinates (q, r). */
const HEX_DIRECTIONS: ReadonlyArray<[number, number]> = [
    [0, -1], // North
    [1, -1], // NorthEast
    [1, 0], // SouthEast
    [0, 1], // South
    [-1, 1], // SouthWest
    [-1, 0] // NorthWest
];

/** Chunk grid coordinate. */
export class ChunkId {
    static readonly ORIGIN = new ChunkId(0, 0);

    constructor(
        readonly q: number,
        readonly r: number
    ) {}

    key(): string {
        return `${this.q},${this.r}`;
    }

    /** Calculate hex distance to another chunk using cube coordinates. */
    distanceTo(other: ChunkId): number {
        const dq = this.q - other.q;
        const dr = this.r - other.r;
        const ds = -this.q - this.r - (-other.q - other.r);
        return (Math.abs(dq) + Math.abs(dr) + Math.abs(ds)) / 2;
    }

    isTracked(reference: ChunkId): boolean {
        return this.distanceTo(reference) <= MAX_TRACKED_CHUNK_DISTANCE;
    }

    isActive(reference: ChunkId): boolean {
        return this.distanceTo(reference) <= MAX_ACTIVE_CHUNK_DISTANCE;
    }

    /** Return the 6 immediate hex neighbors. */
    neighbors(): ChunkId[] {
        return HEX_DIRECTIONS.map(([dq, dr]) => new ChunkId(this.q + dq, this.r + dr));
    }

    /** Return all coordinates on the hex ring at the given radius. */
    ring(radius: number): ChunkId[] {
        if (radius === 0) return [new ChunkId(this.q, this.r)];

        const results: ChunkId[] = [];
        // Start at the "north" corner of the ring
        let q = this.q;
        let r = this.r - radius;

        // Walk along each of the 6 edges
        // Ring directions: SE, S, SW, NW, N, NE (matching Rust RingIterator)
        const ringDirs: ReadonlyArray<[number, number]> = [
            [1, 0], // SouthEast
            [0, 1], // South
            [-1, 1], // SouthWest
            [-1, 0], // NorthWest
            [0, -1], // North
            [1, -1] // NorthEast
        ];

        for (const [dq, dr] of ringDirs) {
            for (let step = 0; step < radius; step++) {
                results.push(new ChunkId(q, r));
                q += dq;
                r += dr;
            }
        }

        return results;
    }

    /** Return all coordinates within the given radius. */
    spiral(radius: number): ChunkId[] {
        return range(0, radius)
            .map((r) => this.ring(r))
            .flatten()
            .toArray();
    }
}

/** Hex axial coordinate. */
export class AxialCoord {
    constructor(
        readonly q: number,
        readonly r: number
    ) {}
    key(): string {
        return `${this.q},${this.r}`;
    }
}
