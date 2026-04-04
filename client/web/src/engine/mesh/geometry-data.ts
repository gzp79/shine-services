/**
 * Quad mesh geometry data (primal mesh)
 */
export class QuadData {
    constructor(
        /** Flat vertex array: [x, y, x, y, ...] for 2D positions */
        public readonly vertices: Float32Array,
        /** Flat quad indices: [a, b, c, d, a, b, c, d, ...] referencing vertices */
        public readonly indices: Uint32Array,
        /** Flat boundary edge indices: [a, b, a, b, ...] pairs of vertex indices (optional) */
        public readonly boundaryIndices?: Uint32Array
    ) {}
}

/**
 * Dual polygon mesh geometry data
 */
export class PolygonData {
    constructor(
        /** Flat vertex array: [x, y, x, y, ...] for 2D positions (dual vertices = quad centers) */
        public readonly vertices: Float32Array,
        /** Flat polygon indices: [idx, idx, ...] referencing vertices, all polygons concatenated */
        public readonly indices: Uint32Array,
        /** Polygon start offsets: [0, n1, n1+n2, ...] where polygon i spans indices[starts[i]..starts[i+1]] */
        public readonly starts: Uint32Array
    ) {}
}
