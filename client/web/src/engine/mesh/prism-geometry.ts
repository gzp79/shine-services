import * as THREE from 'three';
import { PolygonData } from './geometry-data';

const PRISM_HEIGHT = 100.0;

/**
 * Build a prism geometry for a dual polygon.
 * Extrudes the polygon from z=0 to z=PRISM_HEIGHT.
 *
 * @param polygonIndices - Indices into vertices array for this polygon
 * @param vertices - Flat vertex array [x, y, x, y, ...]
 * @returns THREE.BufferGeometry for the prism
 */
export function buildPrismGeometry(polygonIndices: Uint32Array, vertices: Float32Array): THREE.BufferGeometry {
    const n = polygonIndices.length;

    if (n < 3) {
        // Degenerate polygon
        return new THREE.BufferGeometry();
    }

    // Extract polygon vertices
    const polyVerts: Array<[number, number]> = [];
    for (let i = 0; i < n; i++) {
        const idx = polygonIndices[i];
        polyVerts.push([vertices[idx * 2], vertices[idx * 2 + 1]]);
    }

    // Build prism triangles with barycentric coordinates, edge flags, and vertex colors
    const positions: number[] = [];
    const barycentrics: number[] = [];
    const edgeFlags: number[] = [];
    const colors: number[] = []; // RGB vertex colors for bottom/top/sides

    // Bottom face (z=0) - fan triangulation from first vertex (v0 is on perimeter!)
    // Show all perimeter edges
    for (let i = 1; i < n - 1; i++) {
        // Triangle: v0, vi, vi+1
        positions.push(
            polyVerts[0][0],
            polyVerts[0][1],
            0,
            polyVerts[i][0],
            polyVerts[i][1],
            0,
            polyVerts[i + 1][0],
            polyVerts[i + 1][1],
            0
        );

        // Barycentric coords: each vertex gets one component = 1
        barycentrics.push(
            1,
            0,
            0, // v0
            0,
            1,
            0, // vi
            0,
            0,
            1 // vi+1
        );

        // Edge flags for perimeter:
        // - barycentric.x=0 at edge vi-vi+1: ALWAYS show (perimeter)
        // - barycentric.y=0 at edge vi+1-v0: show on LAST triangle (perimeter)
        // - barycentric.z=0 at edge v0-vi: show on FIRST triangle (perimeter)
        const edge0 = 1; // vi-vi+1: always perimeter
        const edge1 = i === n - 2 ? 1 : 0; // vi+1-v0: last triangle only
        const edge2 = i === 1 ? 1 : 0; // v0-vi: first triangle only
        const flags = [edge0, edge1, edge2];
        edgeFlags.push(...flags, ...flags, ...flags);

        // Bottom face color (z=0): bright cyan
        colors.push(0.2, 0.9, 1.0, 0.2, 0.9, 1.0, 0.2, 0.9, 1.0);
    }

    // Top face (z=PRISM_HEIGHT) - fan triangulation, same winding as bottom
    // Show all perimeter edges
    for (let i = 1; i < n - 1; i++) {
        // Triangle: v0, vi, vi+1
        positions.push(
            polyVerts[0][0],
            polyVerts[0][1],
            PRISM_HEIGHT,
            polyVerts[i][0],
            polyVerts[i][1],
            PRISM_HEIGHT,
            polyVerts[i + 1][0],
            polyVerts[i + 1][1],
            PRISM_HEIGHT
        );

        barycentrics.push(
            1,
            0,
            0, // v0
            0,
            1,
            0, // vi
            0,
            0,
            1 // vi+1
        );

        // Edge flags for perimeter (same logic as bottom face)
        const edge0 = 1; // vi-vi+1: always perimeter
        const edge1 = i === n - 2 ? 1 : 0; // vi+1-v0: last triangle only
        const edge2 = i === 1 ? 1 : 0; // v0-vi: first triangle only
        const flags = [edge0, edge1, edge2];
        edgeFlags.push(...flags, ...flags, ...flags);

        // Top face color (z=PRISM_HEIGHT): bright orange
        colors.push(1.0, 0.5, 0.0, 1.0, 0.5, 0.0, 1.0, 0.5, 0.0);
    }

    // Side faces - quads split into triangles
    // Show vertical and horizontal edges, skip diagonals
    for (let i = 0; i < n; i++) {
        const next = (i + 1) % n;
        const [x0, y0] = polyVerts[i];
        const [x1, y1] = polyVerts[next];

        // Triangle 1: BL, BR, TR
        positions.push(x0, y0, 0, x1, y1, 0, x1, y1, PRISM_HEIGHT);

        barycentrics.push(1, 0, 0, 0, 1, 0, 0, 0, 1);

        // Edge flags:
        // - barycentric.x checks BR-TR (right vertical): SHOW
        // - barycentric.y checks TR-BL (diagonal): SKIP
        // - barycentric.z checks BL-BR (bottom horizontal): SHOW
        const flags1 = [1, 0, 1];
        edgeFlags.push(...flags1, ...flags1, ...flags1);

        // Side face triangle 1 colors: BL (bottom), BR (bottom), TR (top)
        colors.push(0.2, 0.9, 1.0, 0.2, 0.9, 1.0, 1.0, 0.5, 0.0);

        // Triangle 2: BL, TR, TL
        positions.push(x0, y0, 0, x1, y1, PRISM_HEIGHT, x0, y0, PRISM_HEIGHT);

        barycentrics.push(1, 0, 0, 0, 1, 0, 0, 0, 1);

        // Edge flags:
        // - barycentric.x checks TR-TL (top horizontal): SHOW
        // - barycentric.y checks TL-BL (left vertical): SHOW
        // - barycentric.z checks BL-TR (diagonal): SKIP
        const flags2 = [1, 1, 0];
        edgeFlags.push(...flags2, ...flags2, ...flags2);

        // Side face triangle 2 colors: BL (bottom), TR (top), TL (top)
        colors.push(0.2, 0.9, 1.0, 1.0, 0.5, 0.0, 1.0, 0.5, 0.0);
    }

    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
    geometry.setAttribute('barycentric', new THREE.Float32BufferAttribute(barycentrics, 3));
    geometry.setAttribute('edgeFlags', new THREE.Float32BufferAttribute(edgeFlags, 3));
    geometry.setAttribute('color', new THREE.Float32BufferAttribute(colors, 3));

    return geometry;
}

/**
 * Build all selection prisms for a chunk.
 * Returns a map of vertIdx -> THREE.Mesh.
 */
export function buildSelectionMeshes(data: PolygonData, chunkId: { q: number; r: number }): Map<number, THREE.Mesh> {
    const meshes = new Map<number, THREE.Mesh>();
    const vertexCount = data.starts.length - 1;

    for (let vi = 0; vi < vertexCount; vi++) {
        const start = data.starts[vi];
        const end = data.starts[vi + 1];

        if (end <= start) {
            // No polygon for this vertex
            continue;
        }

        const polygonIndices = data.indices.slice(start, end);
        const geometry = buildPrismGeometry(polygonIndices, data.vertices);

        if (geometry.attributes.position && geometry.attributes.position.count > 0) {
            const material = new THREE.MeshBasicMaterial({
                color: [0x44aa88, 0x88aa44, 0xaa4488, 0x4488aa][vi % 4],
                transparent: true,
                opacity: 0.6,
                side: THREE.DoubleSide,
                depthWrite: false
            });

            const mesh = new THREE.Mesh(geometry, material);
            mesh.userData = { vertIdx: vi, chunkId, isSelectionMesh: true };

            // Use layer 1 for selection (layer 0 is default visible layer)
            // Camera sees layer 0, raycaster checks layer 1
            mesh.layers.set(1);

            meshes.set(vi, mesh);
        } else {
            geometry.dispose();
        }
    }

    return meshes;
}
