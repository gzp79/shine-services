import * as THREE from 'three';
import type { PolygonMesh } from './polygon-mesh';

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
        return new THREE.BufferGeometry();
    }

    const polyVerts: Array<[number, number]> = [];
    for (let i = 0; i < n; i++) {
        const idx = polygonIndices[i];
        polyVerts.push([vertices[idx * 2], vertices[idx * 2 + 1]]);
    }

    const positions: number[] = [];
    const barycentrics: number[] = [];
    const edgeFlags: number[] = [];
    const colors: number[] = [];

    // Bottom face (z=0) - fan triangulation from first vertex
    for (let i = 1; i < n - 1; i++) {
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
        barycentrics.push(1, 0, 0, 0, 1, 0, 0, 0, 1);
        const edge0 = 1;
        const edge1 = i === n - 2 ? 1 : 0;
        const edge2 = i === 1 ? 1 : 0;
        const flags = [edge0, edge1, edge2];
        edgeFlags.push(...flags, ...flags, ...flags);
        colors.push(0.2, 0.9, 1.0, 0.2, 0.9, 1.0, 0.2, 0.9, 1.0);
    }

    // Top face (z=PRISM_HEIGHT) - fan triangulation
    for (let i = 1; i < n - 1; i++) {
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
        barycentrics.push(1, 0, 0, 0, 1, 0, 0, 0, 1);
        const edge0 = 1;
        const edge1 = i === n - 2 ? 1 : 0;
        const edge2 = i === 1 ? 1 : 0;
        const flags = [edge0, edge1, edge2];
        edgeFlags.push(...flags, ...flags, ...flags);
        colors.push(1.0, 0.5, 0.0, 1.0, 0.5, 0.0, 1.0, 0.5, 0.0);
    }

    // Side faces
    for (let i = 0; i < n; i++) {
        const next = (i + 1) % n;
        const [x0, y0] = polyVerts[i];
        const [x1, y1] = polyVerts[next];

        positions.push(x0, y0, 0, x1, y1, 0, x1, y1, PRISM_HEIGHT);
        barycentrics.push(1, 0, 0, 0, 1, 0, 0, 0, 1);
        const flags1 = [1, 0, 1];
        edgeFlags.push(...flags1, ...flags1, ...flags1);
        colors.push(0.2, 0.9, 1.0, 0.2, 0.9, 1.0, 1.0, 0.5, 0.0);

        positions.push(x0, y0, 0, x1, y1, PRISM_HEIGHT, x0, y0, PRISM_HEIGHT);
        barycentrics.push(1, 0, 0, 0, 1, 0, 0, 0, 1);
        const flags2 = [1, 1, 0];
        edgeFlags.push(...flags2, ...flags2, ...flags2);
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
export function buildSelectionMeshes(data: PolygonMesh, chunkId: { q: number; r: number }): Map<number, THREE.Mesh> {
    const meshes = new Map<number, THREE.Mesh>();
    const polygonCount = data.ranges.length / 2;

    for (let vi = 0; vi < polygonCount; vi++) {
        const start = data.ranges[vi * 2];
        const end = data.ranges[vi * 2 + 1];

        if (end <= start) continue;

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
            mesh.layers.set(1);
            meshes.set(vi, mesh);
        } else {
            geometry.dispose();
        }
    }

    return meshes;
}
