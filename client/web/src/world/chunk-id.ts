import * as THREE from 'three';
import { CHUNK_WORLD_SIZE, MAX_ACTIVE_CHUNK_DISTANCE, MAX_TRACKED_CHUNK_DISTANCE } from '../constants';
import { range } from '../engine/utils';

/**
 * Pointy-top hex grid corner directions. Matches Rust HexPointyDir indices exactly.
 */
export const enum HexPointyDir {
    E = 0,
    NE = 1,
    NW = 2,
    W = 3,
    SW = 4,
    SE = 5
}

/**
 * Flat-top hex grid directions. Matches Rust HexFlatDir indices exactly.
 * Used as argument to ChunkId.neighbor().
 */
export const enum HexFlatDir {
    NE = 0,
    N = 1,
    NW = 2,
    SW = 3,
    S = 4,
    SE = 5
}

/** (dq, dr) deltas indexed by HexFlatDir. */
const HEX_DIRECTIONS: ReadonlyArray<[number, number]> = [
    [1, -1], // NE
    [0, -1], // N
    [-1, 0], // NW
    [-1, 1], // SW
    [0, 1], // S
    [1, 0] // SE
];

/** Ring walk directions (NW→SW→S→SE→NE→N), matching Rust RingIterator. */
const RING_WALK: ReadonlyArray<HexFlatDir> = [
    HexFlatDir.NW,
    HexFlatDir.SW,
    HexFlatDir.S,
    HexFlatDir.SE,
    HexFlatDir.NE,
    HexFlatDir.N
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

    /** Check equality with another chunk coordinate. */
    equals(other: ChunkId): boolean {
        return this.q === other.q && this.r === other.r;
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

    isInteractable(reference: ChunkId): boolean {
        return this.distanceTo(reference) <= MAX_ACTIVE_CHUNK_DISTANCE;
    }

    /** Return the 6 immediate hex neighbors in HexFlatDir order. */
    neighbors(): ChunkId[] {
        return HEX_DIRECTIONS.map(([dq, dr]) => new ChunkId(this.q + dq, this.r + dr));
    }

    /** Get neighbor in the given HexFlatDir direction. */
    neighbor(direction: HexFlatDir): ChunkId {
        const [dq, dr] = HEX_DIRECTIONS[direction];
        return new ChunkId(this.q + dq, this.r + dr);
    }

    /**
     * Return all coordinates on the hex ring at the given radius.
     * Starts at the NE corner, walks NW→SW→S→SE→NE→N — matches Rust RingIterator.
     */
    ring(radius: number): ChunkId[] {
        if (radius === 0) return [new ChunkId(this.q, this.r)];

        const results: ChunkId[] = [];
        // Start at NE corner: center + NE * radius
        const [dq0, dr0] = HEX_DIRECTIONS[HexFlatDir.NE];
        let q = this.q + dq0 * radius;
        let r = this.r + dr0 * radius;

        for (const dir of RING_WALK) {
            const [dq, dr] = HEX_DIRECTIONS[dir];
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

    /** World position of this chunk relative to a reference chunk (matches Rust FlatAxialCoord::to_position). */
    toWorldPosition(reference: ChunkId): THREE.Vector2 {
        const x = CHUNK_WORLD_SIZE * 1.5 * this.q;
        const y = -CHUNK_WORLD_SIZE * (Math.sqrt(3) * this.r + (Math.sqrt(3) / 2) * this.q);
        const refX = CHUNK_WORLD_SIZE * 1.5 * reference.q;
        const refY = -CHUNK_WORLD_SIZE * (Math.sqrt(3) * reference.r + (Math.sqrt(3) / 2) * reference.q);
        return new THREE.Vector2(x - refX, y - refY);
    }

    /** Chunk containing the given world position (position is relative to reference chunk). */
    static fromWorldPosition(reference: ChunkId, worldPos: THREE.Vector2): ChunkId {
        const refX = CHUNK_WORLD_SIZE * 1.5 * reference.q;
        const refY = -CHUNK_WORLD_SIZE * (Math.sqrt(3) * reference.r + (Math.sqrt(3) / 2) * reference.q);
        const absX = worldPos.x + refX;
        const absY = worldPos.y + refY;
        const q = (2 / 3) * (absX / CHUNK_WORLD_SIZE);
        const r = -absY / CHUNK_WORLD_SIZE / Math.sqrt(3) - q / 2;
        return ChunkId.roundAxial(q, r);
    }

    private static roundAxial(q: number, r: number): ChunkId {
        const s = -q - r;
        let rq = Math.round(q);
        let rr = Math.round(r);
        const rs = Math.round(s);
        const qd = Math.abs(rq - q);
        const rd = Math.abs(rr - r);
        const sd = Math.abs(rs - s);
        if (qd > rd && qd > sd) rq = -rr - rs;
        else if (rd > sd) rr = -rq - rs;
        return new ChunkId(rq, rr);
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
