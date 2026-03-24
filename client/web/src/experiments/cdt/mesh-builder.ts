import * as THREE from 'three';

const FILL_COLOR = new THREE.Color(0.82, 0.85, 0.88);
const EDGE_COLOR = 0x555555;
const FIXED_EDGE_COLOR = 0xff4444;
const POINT_COLOR = 0x2288aa;

export interface CdtData {
    vertices: Float32Array;
    triangles: Uint32Array;
    fixedEdges: Uint32Array;
}

export interface CdtMeshGroup {
    group: THREE.Group;
    dispose: () => void;
}

export function buildCdtMesh(data: CdtData): CdtMeshGroup {
    const group = new THREE.Group();
    const vertCount = data.vertices.length / 2;
    const triCount = data.triangles.length / 3;

    // Build 3D positions: (x, 0, y)
    const positions = new Float32Array(vertCount * 3);
    for (let i = 0; i < vertCount; i++) {
        positions[i * 3] = data.vertices[i * 2];
        positions[i * 3 + 1] = 0;
        positions[i * 3 + 2] = data.vertices[i * 2 + 1];
    }

    // Filled triangles
    const triPositions: number[] = [];
    const triColors: number[] = [];
    for (let t = 0; t < triCount; t++) {
        const a = data.triangles[t * 3];
        const b = data.triangles[t * 3 + 1];
        const c = data.triangles[t * 3 + 2];
        for (const idx of [a, b, c]) {
            triPositions.push(positions[idx * 3], positions[idx * 3 + 1], positions[idx * 3 + 2]);
            triColors.push(FILL_COLOR.r, FILL_COLOR.g, FILL_COLOR.b);
        }
    }

    const fillGeom = new THREE.BufferGeometry();
    fillGeom.setAttribute('position', new THREE.Float32BufferAttribute(triPositions, 3));
    fillGeom.setAttribute('color', new THREE.Float32BufferAttribute(triColors, 3));
    const fillMat = new THREE.MeshBasicMaterial({
        vertexColors: true,
        side: THREE.DoubleSide
    });
    const fillMesh = new THREE.Mesh(fillGeom, fillMat);
    group.add(fillMesh);

    // Triangle edge wireframe
    const edgePositions: number[] = [];
    for (let t = 0; t < triCount; t++) {
        const base = t * 3;
        for (let e = 0; e < 3; e++) {
            const i0 = data.triangles[base + e];
            const i1 = data.triangles[base + ((e + 1) % 3)];
            edgePositions.push(
                positions[i0 * 3],
                5,
                positions[i0 * 3 + 2],
                positions[i1 * 3],
                5,
                positions[i1 * 3 + 2]
            );
        }
    }
    const edgeGeom = new THREE.BufferGeometry();
    edgeGeom.setAttribute('position', new THREE.Float32BufferAttribute(edgePositions, 3));
    const edgeMat = new THREE.LineBasicMaterial({ color: EDGE_COLOR });
    const edgeLines = new THREE.LineSegments(edgeGeom, edgeMat);
    group.add(edgeLines);

    // Fixed/constraint edges (highlighted)
    const fixedCount = data.fixedEdges.length / 2;
    if (fixedCount > 0) {
        const fixedPositions: number[] = [];
        for (let e = 0; e < fixedCount; e++) {
            const i0 = data.fixedEdges[e * 2];
            const i1 = data.fixedEdges[e * 2 + 1];
            fixedPositions.push(
                positions[i0 * 3],
                10,
                positions[i0 * 3 + 2],
                positions[i1 * 3],
                10,
                positions[i1 * 3 + 2]
            );
        }
        const fixedGeom = new THREE.BufferGeometry();
        fixedGeom.setAttribute('position', new THREE.Float32BufferAttribute(fixedPositions, 3));
        const fixedMat = new THREE.LineBasicMaterial({ color: FIXED_EDGE_COLOR, linewidth: 2 });
        const fixedLines = new THREE.LineSegments(fixedGeom, fixedMat);
        group.add(fixedLines);
    }

    // Point sprites
    const pointGeom = new THREE.BufferGeometry();
    pointGeom.setAttribute(
        'position',
        new THREE.Float32BufferAttribute(
            Array.from({ length: vertCount }, (_, i) => [positions[i * 3], 15, positions[i * 3 + 2]]).flat(),
            3
        )
    );
    const pointMat = new THREE.PointsMaterial({ color: POINT_COLOR, size: 40, sizeAttenuation: true });
    const pointCloud = new THREE.Points(pointGeom, pointMat);
    group.add(pointCloud);

    const dispose = () => {
        group.traverse((obj) => {
            if (obj instanceof THREE.Mesh || obj instanceof THREE.LineSegments || obj instanceof THREE.Points) {
                obj.geometry.dispose();
                if (obj.material instanceof THREE.Material) obj.material.dispose();
            }
        });
    };

    return { group, dispose };
}
