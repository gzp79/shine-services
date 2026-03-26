/** Chunk grid coordinate. */
export class ChunkId {
    constructor(
        readonly q: number,
        readonly r: number
    ) {}
    key(): string {
        return `${this.q},${this.r}`;
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
