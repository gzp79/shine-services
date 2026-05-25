import * as THREE from 'three';
import { buildPrismGeometry } from './builder';
import type { PolygonMesh } from './polygon-mesh';

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

        const geometry = buildPrismGeometry(data, vi);

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
