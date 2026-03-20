import * as THREE from 'three';

const FILL_COLOR = new THREE.Color(0.82, 0.85, 0.88); // light grey
const EDGE_COLOR = 0x333333;

export interface MeshData {
    vertices: Float32Array;
    indices: Uint32Array;
    patchIndices: Uint8Array;
}

export interface HexMeshGroup {
    group: THREE.Group;
    dispose: () => void;
}

export function buildHexMesh(data: MeshData): HexMeshGroup {
    const group = new THREE.Group();
    const vertCount = data.vertices.length / 2;
    const quadCount = data.indices.length / 4;

    // Build 3D position array: (x, 0, y)
    const positions = new Float32Array(vertCount * 3);
    for (let i = 0; i < vertCount; i++) {
        positions[i * 3] = data.vertices[i * 2]; // x
        positions[i * 3 + 1] = 0; // y (up)
        positions[i * 3 + 2] = data.vertices[i * 2 + 1]; // z (from 2D y)
    }

    // Build non-indexed geometry with per-face vertex colors
    // Each quad (a,b,c,d) → triangles (a,b,c) and (a,c,d)
    const triPositions: number[] = [];
    const triColors: number[] = [];

    for (let q = 0; q < quadCount; q++) {
        const a = data.indices[q * 4];
        const b = data.indices[q * 4 + 1];
        const c = data.indices[q * 4 + 2];
        const d = data.indices[q * 4 + 3];

        // Triangle 1: a, b, c
        for (const idx of [a, b, c]) {
            triPositions.push(positions[idx * 3], positions[idx * 3 + 1], positions[idx * 3 + 2]);
            triColors.push(FILL_COLOR.r, FILL_COLOR.g, FILL_COLOR.b);
        }

        // Triangle 2: a, c, d
        for (const idx of [a, c, d]) {
            triPositions.push(positions[idx * 3], positions[idx * 3 + 1], positions[idx * 3 + 2]);
            triColors.push(FILL_COLOR.r, FILL_COLOR.g, FILL_COLOR.b);
        }
    }

    // Filled mesh
    const fillGeom = new THREE.BufferGeometry();
    fillGeom.setAttribute('position', new THREE.Float32BufferAttribute(triPositions, 3));
    fillGeom.setAttribute('color', new THREE.Float32BufferAttribute(triColors, 3));
    fillGeom.computeVertexNormals();

    const fillMat = new THREE.MeshStandardMaterial({
        vertexColors: true,
        flatShading: true,
        side: THREE.DoubleSide
    });
    const fillMesh = new THREE.Mesh(fillGeom, fillMat);
    group.add(fillMesh);

    // Edge lines — explicitly from quad topology
    const edgePositions: number[] = [];
    for (let q = 0; q < quadCount; q++) {
        const qi = q * 4;
        for (let e = 0; e < 4; e++) {
            const i0 = data.indices[qi + e];
            const i1 = data.indices[qi + ((e + 1) % 4)];
            edgePositions.push(
                positions[i0 * 3],
                positions[i0 * 3 + 1] + 0.01,
                positions[i0 * 3 + 2],
                positions[i1 * 3],
                positions[i1 * 3 + 1] + 0.01,
                positions[i1 * 3 + 2]
            );
        }
    }

    const edgeGeom = new THREE.BufferGeometry();
    edgeGeom.setAttribute('position', new THREE.Float32BufferAttribute(edgePositions, 3));
    const edgeMat = new THREE.LineBasicMaterial({ color: EDGE_COLOR });
    const edgeLines = new THREE.LineSegments(edgeGeom, edgeMat);
    group.add(edgeLines);

    const dispose = () => {
        fillGeom.dispose();
        fillMat.dispose();
        edgeGeom.dispose();
        edgeMat.dispose();
    };

    return { group, dispose };
}
