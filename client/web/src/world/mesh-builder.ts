import * as THREE from 'three';

const DEFAULT_FILL_COLOR = new THREE.Color(0.82, 0.85, 0.88);
const BORDER_COLOR = 0x333333;

export interface MeshData {
    vertices(): Float32Array;
    quadIndices(): Uint32Array;
    borderIndices(): Uint32Array;
}

export interface MeshBuilder {
    group: THREE.Group;
    dispose(): void;
}

export function buildMesh(data: MeshData, fillColor?: THREE.Color): MeshBuilder {
    const color = fillColor ?? DEFAULT_FILL_COLOR;
    const group = new THREE.Group();

    const vertices2D = data.vertices();
    const quadIndices = data.quadIndices();
    const borderIndices = data.borderIndices();

    const vertCount = vertices2D.length / 2;
    const quadCount = quadIndices.length / 4;

    // Build 3D positions: (x, y, 0) from 2D (x, y) — XY is the ground plane, Z is up
    const positions = new Float32Array(vertCount * 3);
    for (let i = 0; i < vertCount; i++) {
        positions[i * 3] = vertices2D[i * 2]; // x
        positions[i * 3 + 1] = vertices2D[i * 2 + 1]; // y
        positions[i * 3 + 2] = 0; // z
    }

    // Build non-indexed triangle geometry from quads
    // Each quad (a,b,c,d) -> triangles (a,b,c) and (a,c,d)
    const triPositions: number[] = [];
    const triColors: number[] = [];

    for (let q = 0; q < quadCount; q++) {
        const a = quadIndices[q * 4];
        const b = quadIndices[q * 4 + 1];
        const c = quadIndices[q * 4 + 2];
        const d = quadIndices[q * 4 + 3];

        for (const idx of [a, b, c, a, c, d]) {
            triPositions.push(positions[idx * 3], positions[idx * 3 + 1], positions[idx * 3 + 2]);
            triColors.push(color.r, color.g, color.b);
        }
    }

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

    // Build border outline from border edge indices
    const borderEdgeCount = borderIndices.length / 2;
    const borderPositions: number[] = [];
    for (let e = 0; e < borderEdgeCount; e++) {
        const i0 = borderIndices[e * 2];
        const i1 = borderIndices[e * 2 + 1];
        borderPositions.push(
            positions[i0 * 3],
            positions[i0 * 3 + 1],
            positions[i0 * 3 + 2] + 0.01,
            positions[i1 * 3],
            positions[i1 * 3 + 1],
            positions[i1 * 3 + 2] + 0.01
        );
    }

    const borderGeom = new THREE.BufferGeometry();
    borderGeom.setAttribute('position', new THREE.Float32BufferAttribute(borderPositions, 3));
    const borderMat = new THREE.LineBasicMaterial({ color: BORDER_COLOR });
    const borderLines = new THREE.LineSegments(borderGeom, borderMat);
    group.add(borderLines);

    return {
        group,
        dispose() {
            fillGeom.dispose();
            fillMat.dispose();
            borderGeom.dispose();
            borderMat.dispose();
        }
    };
}
