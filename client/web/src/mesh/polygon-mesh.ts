export type PolygonMesh = {
    readonly vertices: Float32Array;
    readonly indices: Uint32Array;
    readonly ranges: Uint32Array;
};

export type WiredPolygonMesh = PolygonMesh & {
    readonly wireIndices: Uint32Array;
    readonly wireRanges: Uint32Array;
};
