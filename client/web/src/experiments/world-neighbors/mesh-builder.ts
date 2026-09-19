import { HexFlatDir, HexPointyDir, World, hex_flat_neighbor } from '#wasm';
import * as THREE from 'three';
import { ManagedMesh } from '../../engine/resources/managed-mesh';
import { type ToggleableGroup, createToggleableGroup } from '../../engine/scene/toggleable-group';
import { type PolygonMeshSource, asPolygonMesh } from '../../mesh/polygon-mesh';

const EDGE_COLOR = 0x222222;

export interface ChunkCoord {
    q: number;
    r: number;
}

/** Center chunk followed by its 6 flat-top neighbors, in HexFlatDir order (index 0 = center). */
export function neighborChunkIds(center: ChunkCoord): ChunkCoord[] {
    const ids: ChunkCoord[] = [center];
    for (let dir = 0; dir < 6; dir++) {
        const n = hex_flat_neighbor(center.q, center.r, dir as HexFlatDir);
        ids.push({ q: n[0], r: n[1] });
    }
    return ids;
}

// Build a colored polygon mesh from vertices/indices/ranges with z-offset
function buildPolygonMesh(
    vertices: Float32Array,
    indices: Uint32Array,
    ranges: Uint32Array,
    color: THREE.Color,
    zOffset: number
): THREE.Mesh {
    const positions: number[] = [];
    const colors: number[] = [];

    for (let p = 0; p < ranges.length; p += 2) {
        const start = ranges[p];
        const end = ranges[p + 1];
        const polySize = end - start;

        if (polySize < 3) continue; // Skip degenerate polygons

        // Fan triangulation from first vertex
        const firstIdx = indices[start];
        for (let i = 1; i < polySize - 1; i++) {
            const idx1 = indices[start + i];
            const idx2 = indices[start + i + 1];

            for (const idx of [firstIdx, idx1, idx2]) {
                positions.push(vertices[idx * 2], vertices[idx * 2 + 1], zOffset);
                colors.push(color.r, color.g, color.b);
            }
        }
    }

    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
    geom.setAttribute('color', new THREE.Float32BufferAttribute(colors, 3));
    geom.computeVertexNormals();

    const mat = new THREE.MeshStandardMaterial({
        vertexColors: true,
        flatShading: true,
        side: THREE.DoubleSide
    });

    return ManagedMesh.own(geom, mat);
}

// Build wireframe for polygons with z-offset
function buildPolygonWireframe(
    vertices: Float32Array,
    indices: Uint32Array,
    ranges: Uint32Array,
    zOffset: number
): THREE.LineSegments {
    const positions: number[] = [];

    for (let p = 0; p < ranges.length; p += 2) {
        const start = ranges[p];
        const end = ranges[p + 1];
        const polySize = end - start;

        if (polySize < 2) continue;

        for (let i = 0; i < polySize; i++) {
            const idx0 = indices[start + i];
            const idx1 = indices[start + ((i + 1) % polySize)];
            positions.push(
                vertices[idx0 * 2],
                vertices[idx0 * 2 + 1],
                zOffset,
                vertices[idx1 * 2],
                vertices[idx1 * 2 + 1],
                zOffset
            );
        }
    }

    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
    const mat = new THREE.LineBasicMaterial({ color: EDGE_COLOR });

    return new THREE.LineSegments(geom, mat);
}

// Filled mesh + wireframe for one cell polygon set, taken straight from a cell handle.
function buildCellGroup(source: PolygonMeshSource, color: THREE.Color, meshZ: number, wireZ: number): THREE.Group {
    const group = new THREE.Group();
    const mesh = asPolygonMesh(source);
    const { vertices, indices, ranges } = mesh;

    if (vertices.length > 0) {
        group.add(buildPolygonMesh(vertices, indices, ranges, color, meshZ));
        group.add(buildPolygonWireframe(vertices, indices, ranges, wireZ));
    }

    return group;
}

export function buildChunkHexagons(world: World, center: ChunkCoord): THREE.Group {
    const group = new THREE.Group();
    const color = new THREE.Color();

    neighborChunkIds(center).forEach((id, chunkIdx) => {
        using chunk = world.chunk(id.q, id.r);
        const hexVerts = chunk?.hex_vertices();
        if (!hexVerts || hexVerts.length !== 12) return;

        const offset = world.chunk_world_offset(center.q, center.r, id.q, id.r);
        color.setHSL(chunkIdx / 7, 0.5, 0.6);

        // Line loop over the 6 corners, offset into world space and closed back to the first.
        const positions: number[] = [];
        for (let i = 0; i <= 6; i++) {
            const c = i % 6;
            positions.push(hexVerts[c * 2] + offset[0], hexVerts[c * 2 + 1] + offset[1], 2.0);
        }

        const geom = new THREE.BufferGeometry();
        geom.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
        const mat = new THREE.LineBasicMaterial({ color: color.getHex() });
        group.add(new THREE.Line(geom, mat));
    });

    return group;
}

export function buildInteriorMeshes(world: World, center: ChunkCoord): ToggleableGroup {
    const group = new THREE.Group();
    const meshGroups: THREE.Group[] = [];
    const color = new THREE.Color();

    neighborChunkIds(center).forEach((id, chunkIdx) => {
        const chunkGroup = new THREE.Group();
        using chunk = world.chunk(id.q, id.r);

        if (chunk) {
            using cells = chunk.inner_cells();
            if (cells) {
                color.setHSL(chunkIdx / 7, 0.7, 0.5);
                chunkGroup.add(buildCellGroup(cells, color, 0.0, 1.0));
            }
            const offset = world.chunk_world_offset(center.q, center.r, id.q, id.r);
            chunkGroup.position.set(offset[0], offset[1], 0);
        }

        meshGroups.push(chunkGroup);
        group.add(chunkGroup);
    });

    return createToggleableGroup(group, meshGroups);
}

export function buildEdgeMeshes(world: World, center: ChunkCoord): ToggleableGroup {
    const group = new THREE.Group();
    const meshGroups: THREE.Group[] = [];
    const color = new THREE.Color();
    using chunk = world.chunk(center.q, center.r);

    for (let edgeIdx = 0; edgeIdx < 6; edgeIdx++) {
        const edgeGroup = new THREE.Group();
        if (chunk) {
            using cells = chunk.edge_cells(edgeIdx as HexFlatDir);
            if (cells) {
                color.setHSL(edgeIdx / 6, 0.8, 0.5);
                edgeGroup.add(buildCellGroup(cells, color, 0.2, 1.2));
            }
        }
        meshGroups.push(edgeGroup);
        group.add(edgeGroup);
    }

    return createToggleableGroup(group, meshGroups);
}

export function buildCornerMeshes(world: World, center: ChunkCoord): ToggleableGroup {
    const group = new THREE.Group();
    const meshGroups: THREE.Group[] = [];
    const color = new THREE.Color();
    using chunk = world.chunk(center.q, center.r);

    for (let cornerIdx = 0; cornerIdx < 6; cornerIdx++) {
        const cornerGroup = new THREE.Group();
        if (chunk) {
            using cells = chunk.corner_cells(cornerIdx as HexPointyDir);
            if (cells) {
                color.setHSL(cornerIdx / 6, 0.8, 0.4);
                cornerGroup.add(buildCellGroup(cells, color, 0.4, 1.4));
            }
        }
        meshGroups.push(cornerGroup);
        group.add(cornerGroup);
    }

    return createToggleableGroup(group, meshGroups);
}
