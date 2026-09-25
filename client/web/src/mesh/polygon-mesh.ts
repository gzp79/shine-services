export type PolygonMeshLike = {
    readonly vertices: Float32Array;
    readonly indices: Uint32Array;
    readonly ranges: Uint32Array;
};

export type PolygonMeshSource = {
    vertices(): Float32Array | undefined;
    indices(): Uint32Array | undefined;
    ranges(): Uint32Array | undefined;
};

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

export type WiredPolygonMeshLike = PolygonMeshLike & {
    readonly wireIndices: Uint32Array;
    readonly wireRanges: Uint32Array;
};

export type WiredPolygonMeshSource = PolygonMeshSource & {
    wire_indices(): Uint32Array;
    wire_ranges(): Uint32Array;
};

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

export type TileDistortionSource = {
    tile_distortions(): Float32Array | undefined;
};


export type TileDistortionLike = {
    readonly distortions: Float32Array;
};

export function asTileDistortion(source: TileDistortionSource): TileDistortionLike {
    return {
        get distortions() {
            return fresh(source.tile_distortions());
        }
    };
}

export function asTileOutlineMesh(source: TileDistortionSource): PolygonMeshLike {
    let topology: { indices: Uint32Array; ranges: Uint32Array } | null = null;

    function computeTopology(distortion: Float32Array): { indices: Uint32Array; ranges: Uint32Array } {
        const tileCount = distortion.length / 8;
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
            const distortion = fresh(source.tile_distortions());
            topology ??= computeTopology(distortion);
            return topology.indices;
        },
        get ranges() {
            const distortion = fresh(source.tile_distortions());
            topology ??= computeTopology(distortion);
            return topology.ranges;
        }
    };
}

function fresh<T>(value: T | undefined): T {
    if (value === undefined) {
        throw new Error('cell view is stale: its source chunk changed; re-acquire the handle');
    }
    return value;
}