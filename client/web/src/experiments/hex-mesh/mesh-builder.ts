import * as THREE from 'three';

const FILL_COLOR = new THREE.Color(0.82, 0.85, 0.88); // light grey
const EDGE_COLOR = 0x333333;
const DUAL_EDGE_COLOR = 0x2288aa;

export interface MeshData {
    vertices: Float32Array;
    indices: Uint32Array;
    patchIndices: Uint8Array;
    dualVertices: Float32Array;
    dualIndices: Uint32Array;
    anchorIndices: Uint32Array;
    anchorEdgeStarts: Uint32Array;
}

export interface HexMeshGroup {
    group: THREE.Group;
    setPrimalVisible: (visible: boolean) => void;
    setDualVisible: (visible: boolean) => void;
    setAnchorVisible: (visible: boolean) => void;
    setAnchorVerticesVisible: (visible: boolean) => void;
    dispose: () => void;
}

export function buildHexMesh(data: MeshData): HexMeshGroup {
    const group = new THREE.Group();
    const vertCount = data.vertices.length / 2;
    const quadCount = data.indices.length / 4;

    // Build 3D position array: (x, y, 0) — XY is the ground plane, Z is up
    const positions = new Float32Array(vertCount * 3);
    for (let i = 0; i < vertCount; i++) {
        positions[i * 3] = data.vertices[i * 2]; // x
        positions[i * 3 + 1] = data.vertices[i * 2 + 1]; // y
        positions[i * 3 + 2] = 0; // z
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

    // Primal edge lines — explicitly from quad topology
    const edgePositions: number[] = [];
    for (let q = 0; q < quadCount; q++) {
        const qi = q * 4;
        for (let e = 0; e < 4; e++) {
            const i0 = data.indices[qi + e];
            const i1 = data.indices[qi + ((e + 1) % 4)];
            edgePositions.push(
                positions[i0 * 3],
                positions[i0 * 3 + 1],
                positions[i0 * 3 + 2] + 0.01,
                positions[i1 * 3],
                positions[i1 * 3 + 1],
                positions[i1 * 3 + 2] + 0.01
            );
        }
    }

    const edgeGeom = new THREE.BufferGeometry();
    edgeGeom.setAttribute('position', new THREE.Float32BufferAttribute(edgePositions, 3));
    const edgeMat = new THREE.LineBasicMaterial({ color: EDGE_COLOR });
    const edgeLines = new THREE.LineSegments(edgeGeom, edgeMat);
    group.add(edgeLines);

    // Dual edge lines — connect centroids of adjacent quads
    const dualVertCount = data.dualVertices.length / 2;
    const dualPositions3D = new Float32Array(dualVertCount * 3);
    for (let i = 0; i < dualVertCount; i++) {
        dualPositions3D[i * 3] = data.dualVertices[i * 2];
        dualPositions3D[i * 3 + 1] = data.dualVertices[i * 2 + 1];
        dualPositions3D[i * 3 + 2] = 0;
    }

    const dualEdgeCount = data.dualIndices.length / 2;
    const dualEdgePositions: number[] = [];
    for (let e = 0; e < dualEdgeCount; e++) {
        const i0 = data.dualIndices[e * 2];
        const i1 = data.dualIndices[e * 2 + 1];
        dualEdgePositions.push(
            dualPositions3D[i0 * 3],
            dualPositions3D[i0 * 3 + 1],
            dualPositions3D[i0 * 3 + 2] + 0.02,
            dualPositions3D[i1 * 3],
            dualPositions3D[i1 * 3 + 1],
            dualPositions3D[i1 * 3 + 2] + 0.02
        );
    }

    const dualEdgeGeom = new THREE.BufferGeometry();
    dualEdgeGeom.setAttribute('position', new THREE.Float32BufferAttribute(dualEdgePositions, 3));
    const dualEdgeMat = new THREE.LineBasicMaterial({ color: DUAL_EDGE_COLOR });
    const dualEdgeLines = new THREE.LineSegments(dualEdgeGeom, dualEdgeMat);
    dualEdgeLines.visible = false;
    group.add(dualEdgeLines);

    // Anchor edge lines — original boundary edges before subdivision
    // The anchorEdgeStarts array tells us where each anchor edge begins
    const anchorEdgePositions: number[] = [];
    const anchorEdgeColors: number[] = [];
    const anchorVertexPositions: number[] = [];
    const anchorVertexColors: number[] = [];
    const anchorVertexSet = new Set<number>(); // Track unique vertices

    const anchorEdgeCount = data.anchorEdgeStarts.length - 1;
    const color = new THREE.Color();

    for (let edgeIdx = 0; edgeIdx < anchorEdgeCount; edgeIdx++) {
        // Use HSL color wheel for distinct colors per anchor edge
        const hue = edgeIdx / anchorEdgeCount;
        color.setHSL(hue, 1.0, 0.5);

        // Get segment range for this anchor edge
        const segmentStart = data.anchorEdgeStarts[edgeIdx];
        const segmentEnd = data.anchorEdgeStarts[edgeIdx + 1];

        // Add all segments for this anchor edge with same color
        for (let segIdx = segmentStart; segIdx < segmentEnd; segIdx++) {
            const i0 = data.anchorIndices[segIdx * 2];
            const i1 = data.anchorIndices[segIdx * 2 + 1];

            anchorEdgePositions.push(
                positions[i0 * 3],
                positions[i0 * 3 + 1],
                positions[i0 * 3 + 2] + 0.03,
                positions[i1 * 3],
                positions[i1 * 3 + 1],
                positions[i1 * 3 + 2] + 0.03
            );

            // Same color for both vertices of the line segment
            anchorEdgeColors.push(color.r, color.g, color.b);
            anchorEdgeColors.push(color.r, color.g, color.b);

            // Collect unique vertices for markers
            if (!anchorVertexSet.has(i0)) {
                anchorVertexSet.add(i0);
                anchorVertexPositions.push(positions[i0 * 3], positions[i0 * 3 + 1], positions[i0 * 3 + 2] + 0.04);
                anchorVertexColors.push(color.r, color.g, color.b);
            }
            if (!anchorVertexSet.has(i1)) {
                anchorVertexSet.add(i1);
                anchorVertexPositions.push(positions[i1 * 3], positions[i1 * 3 + 1], positions[i1 * 3 + 2] + 0.04);
                anchorVertexColors.push(color.r, color.g, color.b);
            }
        }
    }

    const anchorEdgeGeom = new THREE.BufferGeometry();
    anchorEdgeGeom.setAttribute('position', new THREE.Float32BufferAttribute(anchorEdgePositions, 3));
    anchorEdgeGeom.setAttribute('color', new THREE.Float32BufferAttribute(anchorEdgeColors, 3));
    const anchorEdgeMat = new THREE.LineBasicMaterial({ vertexColors: true, linewidth: 2 });
    const anchorEdgeLines = new THREE.LineSegments(anchorEdgeGeom, anchorEdgeMat);
    anchorEdgeLines.visible = false;
    group.add(anchorEdgeLines);

    // Anchor vertex markers — small circles at each vertex along anchor edges
    // Use instanced mesh for better performance and control over size
    const circleGeom = new THREE.CircleGeometry(0.015, 8); // radius 0.015 world units
    const anchorVertexMat = new THREE.MeshBasicMaterial({ vertexColors: true, side: THREE.DoubleSide });
    const anchorVertexCount = anchorVertexPositions.length / 3;
    const anchorVertexPoints = new THREE.InstancedMesh(circleGeom, anchorVertexMat, anchorVertexCount);

    const matrix = new THREE.Matrix4();
    const vertexColor = new THREE.Color();

    for (let i = 0; i < anchorVertexCount; i++) {
        const x = anchorVertexPositions[i * 3];
        const y = anchorVertexPositions[i * 3 + 1];
        const z = anchorVertexPositions[i * 3 + 2];

        matrix.makeTranslation(x, y, z);
        anchorVertexPoints.setMatrixAt(i, matrix);

        vertexColor.setRGB(anchorVertexColors[i * 3], anchorVertexColors[i * 3 + 1], anchorVertexColors[i * 3 + 2]);
        anchorVertexPoints.setColorAt(i, vertexColor);
    }

    anchorVertexPoints.visible = false;
    group.add(anchorVertexPoints);

    const dispose = () => {
        fillGeom.dispose();
        fillMat.dispose();
        edgeGeom.dispose();
        edgeMat.dispose();
        dualEdgeGeom.dispose();
        dualEdgeMat.dispose();
        anchorEdgeGeom.dispose();
        anchorEdgeMat.dispose();
        circleGeom.dispose();
        anchorVertexMat.dispose();
        anchorVertexPoints.dispose();
    };

    return {
        group,
        setPrimalVisible: (visible: boolean) => {
            edgeLines.visible = visible;
        },
        setDualVisible: (visible: boolean) => {
            dualEdgeLines.visible = visible;
        },
        setAnchorVisible: (visible: boolean) => {
            anchorEdgeLines.visible = visible;
        },
        setAnchorVerticesVisible: (visible: boolean) => {
            anchorVertexPoints.visible = visible;
        },
        dispose
    };
}
