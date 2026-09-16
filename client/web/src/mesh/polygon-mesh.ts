/** Structural shape of polygon mesh data. A borrowed instance is valid only while its owner is unchanged. */
export type PolygonMeshLike = {
    readonly vertices: Float32Array;
    readonly indices: Uint32Array;
    readonly ranges: Uint32Array;
};

export type WiredPolygonMeshLike = PolygonMeshLike & {
    readonly wireIndices: Uint32Array;
    readonly wireRanges: Uint32Array;
};

export type PolygonMeshSource = {
    vertices(): Float32Array | undefined;
    indices(): Uint32Array | undefined;
    ranges(): Uint32Array | undefined;
};

/** Unwraps a cell-view accessor, throwing when its source chunk changed and the view went stale. */
function fresh<T>(value: T | undefined): T {
    if (value === undefined) {
        throw new Error('cell view is stale: its source chunk changed; re-acquire the handle');
    }
    return value;
}

/** Adapts a `PolygonMeshSource` to a `PolygonMeshLike`. */
export function asPolygonMesh(source: PolygonMeshSource): PolygonMeshLike {
    return {
        get vertices() {
            return fresh(source.vertices());
        },
        get indices() {
            return fresh(source.indices());
        },
        get ranges() {
            return fresh(source.ranges());
        }
    };
}

export type WiredPolygonMeshSource = PolygonMeshSource & {
    wire_indices(): Uint32Array;
    wire_ranges(): Uint32Array;
};

/** Adapts a `WiredPolygonMeshSource` to a `WiredPolygonMeshLike`. */
export function asWiredPolygonMesh(source: WiredPolygonMeshSource): WiredPolygonMeshLike {
    return {
        get vertices() {
            return fresh(source.vertices());
        },
        get indices() {
            return fresh(source.indices());
        },
        get ranges() {
            return fresh(source.ranges());
        },
        get wireIndices() {
            return source.wire_indices();
        },
        get wireRanges() {
            return source.wire_ranges();
        }
    };
}

/** Source of the per-tile-quad corner positions `asTileOutlineMesh` is built from. */
export type TileDistortionSource = {
    tile_ids(): Uint32Array | undefined;
    tile_distortions(): Float32Array | undefined;
};

/** Adapts a `TileDistortionSource` to a `PolygonMeshLike` of its tile quads. */
export function asTileOutlineMesh(source: TileDistortionSource): PolygonMeshLike {
    let topology: { indices: Uint32Array; ranges: Uint32Array } | null = null;

    function computeTopology(): { indices: Uint32Array; ranges: Uint32Array } {
        const tileCount = fresh(source.tile_ids()).length;
        const indices = new Uint32Array(tileCount * 4);
        const ranges = new Uint32Array(tileCount * 2);
        for (let i = 0; i < tileCount; i++) {
            for (let c = 0; c < 4; c++) indices[i * 4 + c] = i * 4 + c;
            ranges[i * 2] = i * 4;
            ranges[i * 2 + 1] = i * 4 + 4;
        }
        return { indices, ranges };
    }

    return {
        get vertices() {
            return fresh(source.tile_distortions());
        },
        get indices() {
            topology ??= computeTopology();
            return topology.indices;
        },
        get ranges() {
            topology ??= computeTopology();
            return topology.ranges;
        }
    };
}
