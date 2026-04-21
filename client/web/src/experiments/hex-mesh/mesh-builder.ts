import * as THREE from 'three';
import { disposeMesh } from '../experiment';

const EDGE_COLOR = 0x222222;
const DUAL_EDGE_COLOR = 0x222222;

export interface MeshData {
    // packed as [x0, y0, x1, y1, ...] where each pair is a vertex position in 2D
    vertices: Float32Array;
    // packed as [a0, b0, c0, d0, a1, b1, c1, d1, ...] where each group of 4 is a quad defined by vertex indices
    quad_indices: Uint32Array;
    // packed as [i0, i1, i2, ...] where each value is an index into the vertex array for anchor points
    anchor_indices: Uint32Array;
    // packed as [start0, start1, ...] where each value is the start index of an anchor edge in the anchor_indices array
    anchor_edge_starts: Uint32Array;
    // packed as [x0, y0, x1, y1, ...] where each pair is a dual vertex position in 2D
    dual_vertices: Float32Array;
    // packed as [a0, b0, c0, ...] where each value is an index into the dual_vertices array for dual edges
    dual_indices: Uint32Array;
    // packed as [start0, start1, ...] where each value is the start index of a dual polygon in the dual_indices array
    dual_polygon_starts: Uint32Array;
}

export interface HexMeshGroup {
    group: THREE.Group;
    setPrimalMeshVisible: (visible: boolean) => void;
    setPrimalWireVisible: (visible: boolean) => void;
    setDualMeshVisible: (visible: boolean) => void;
    setDualWireVisible: (visible: boolean) => void;
    setAnchorVisible: (visible: boolean) => void;
    setAnchorVerticesVisible: (visible: boolean) => void;
    dispose: () => void;
}

function buildPrimalMesh(data: MeshData): THREE.Mesh {
    const quadCount = data.quad_indices.length / 4;
    const primalPositions: number[] = [];
    const primalColors: number[] = [];
    const color = new THREE.Color();

    for (let q = 0; q < quadCount; q++) {
        const a = data.quad_indices[q * 4];
        const b = data.quad_indices[q * 4 + 1];
        const c = data.quad_indices[q * 4 + 2];
        const d = data.quad_indices[q * 4 + 3];

        // Generate a random color for this quad using HSL for better distribution
        const hue = (q * 0.618033988749895) % 1.0; // Golden ratio for nice distribution
        color.setHSL(hue, 0.6, 0.65);

        // Triangle 1: a, b, c
        for (const idx of [a, b, c]) {
            primalPositions.push(data.vertices[idx * 2], data.vertices[idx * 2 + 1], 0);
            primalColors.push(color.r, color.g, color.b);
        }

        // Triangle 2: a, c, d
        for (const idx of [a, c, d]) {
            primalPositions.push(data.vertices[idx * 2], data.vertices[idx * 2 + 1], 0);
            primalColors.push(color.r, color.g, color.b);
        }
    }

    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.Float32BufferAttribute(primalPositions, 3));
    geom.setAttribute('color', new THREE.Float32BufferAttribute(primalColors, 3));
    geom.computeVertexNormals();
    const mat = new THREE.MeshStandardMaterial({
        vertexColors: true,
        flatShading: true,
        side: THREE.DoubleSide
    });
    return new THREE.Mesh(geom, mat);
}

function buildPrimalWire(data: MeshData): THREE.LineSegments {
    const quadCount = data.quad_indices.length / 4;
    const positions: number[] = [];

    for (let q = 0; q < quadCount; q++) {
        const qi = q * 4;
        for (let e = 0; e < 4; e++) {
            const i0 = data.quad_indices[qi + e];
            const i1 = data.quad_indices[qi + ((e + 1) % 4)];
            positions.push(
                data.vertices[i0 * 2],
                data.vertices[i0 * 2 + 1],
                0.01,
                data.vertices[i1 * 2],
                data.vertices[i1 * 2 + 1],
                0.01
            );
        }
    }

    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
    const mat = new THREE.LineBasicMaterial({ color: EDGE_COLOR });
    return new THREE.LineSegments(geom, mat);
}

function buildDualMesh(data: MeshData): THREE.Mesh {
    const dualPolygonCount = data.dual_polygon_starts.length;
    const positions: number[] = [];
    const colors: number[] = [];
    const color = new THREE.Color();

    for (let p = 0; p < dualPolygonCount; p++) {
        const start = data.dual_polygon_starts[p];
        const end = p + 1 < dualPolygonCount ? data.dual_polygon_starts[p + 1] : data.dual_indices.length;
        const polySize = end - start;

        if (polySize < 3) continue; // Skip degenerate polygons

        // Generate color for this dual polygon
        const hue = (p * 0.618033988749895) % 1.0;
        color.setHSL(hue, 0.7, 0.5);

        // Fan triangulation from first vertex
        const firstIdx = data.dual_indices[start];
        for (let i = 1; i < polySize - 1; i++) {
            const idx1 = data.dual_indices[start + i];
            const idx2 = data.dual_indices[start + i + 1];

            for (const idx of [firstIdx, idx1, idx2]) {
                positions.push(data.dual_vertices[idx * 2], data.dual_vertices[idx * 2 + 1], 0.001);
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
    const mesh = new THREE.Mesh(geom, mat);
    mesh.visible = false;
    return mesh;
}

function buildDualWire(data: MeshData): THREE.LineSegments {
    const dualPolygonCount = data.dual_polygon_starts.length;
    const positions: number[] = [];

    for (let p = 0; p < dualPolygonCount; p++) {
        const start = data.dual_polygon_starts[p];
        const end = p + 1 < dualPolygonCount ? data.dual_polygon_starts[p + 1] : data.dual_indices.length;
        const polySize = end - start;

        if (polySize < 2) continue;

        for (let i = 0; i < polySize; i++) {
            const idx0 = data.dual_indices[start + i];
            const idx1 = data.dual_indices[start + ((i + 1) % polySize)];
            positions.push(
                data.dual_vertices[idx0 * 2],
                data.dual_vertices[idx0 * 2 + 1],
                0.01,
                data.dual_vertices[idx1 * 2],
                data.dual_vertices[idx1 * 2 + 1],
                0.01
            );
        }
    }

    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
    const mat = new THREE.LineBasicMaterial({ color: DUAL_EDGE_COLOR });
    const lines = new THREE.LineSegments(geom, mat);
    lines.visible = false;
    return lines;
}

function buildAnchorWire(data: MeshData): THREE.LineSegments {
    const anchorEdgeCount = data.anchor_edge_starts.length;
    const positions: number[] = [];
    const colors: number[] = [];
    const color = new THREE.Color();

    for (let a = 0; a < anchorEdgeCount; a++) {
        const start = data.anchor_edge_starts[a];
        const end = a + 1 < anchorEdgeCount ? data.anchor_edge_starts[a + 1] : data.anchor_indices.length;
        const edgeSize = end - start;

        if (edgeSize < 2) continue;

        // Use HSL color wheel for distinct colors per anchor edge
        const hue = (a / anchorEdgeCount) % 1.0;
        color.setHSL(hue, 1.0, 0.5);

        // Create line segments for this anchor edge
        for (let i = 0; i < edgeSize - 1; i++) {
            const idx0 = data.anchor_indices[start + i];
            const idx1 = data.anchor_indices[start + i + 1];
            positions.push(
                data.vertices[idx0 * 2],
                data.vertices[idx0 * 2 + 1],
                0.01,
                data.vertices[idx1 * 2],
                data.vertices[idx1 * 2 + 1],
                0.01
            );
            colors.push(color.r, color.g, color.b);
            colors.push(color.r, color.g, color.b);
        }
    }

    const geom = new THREE.BufferGeometry();
    geom.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
    geom.setAttribute('color', new THREE.Float32BufferAttribute(colors, 3));
    const mat = new THREE.LineBasicMaterial({ vertexColors: true, linewidth: 2 });
    const lines = new THREE.LineSegments(geom, mat);
    lines.visible = false;
    return lines;
}

function buildAnchorVertices(data: MeshData): THREE.InstancedMesh {
    const anchorEdgeCount = data.anchor_edge_starts.length;
    const positions: number[] = [];
    const colors: number[] = [];
    const vertexSet = new Set<number>();
    const color = new THREE.Color();

    for (let a = 0; a < anchorEdgeCount; a++) {
        const start = data.anchor_edge_starts[a];
        const end = a + 1 < anchorEdgeCount ? data.anchor_edge_starts[a + 1] : data.anchor_indices.length;

        const hue = (a / anchorEdgeCount) % 1.0;
        color.setHSL(hue, 1.0, 0.5);

        for (let i = start; i < end; i++) {
            const idx = data.anchor_indices[i];
            if (!vertexSet.has(idx)) {
                vertexSet.add(idx);
                positions.push(data.vertices[idx * 2], data.vertices[idx * 2 + 1], 0.01);
                colors.push(color.r, color.g, color.b);
            }
        }
    }

    const circleGeom = new THREE.CircleGeometry(0.015, 8);
    const mat = new THREE.MeshBasicMaterial({ vertexColors: true, side: THREE.DoubleSide });
    const vertexCount = positions.length / 3;
    const instancedMesh = new THREE.InstancedMesh(circleGeom, mat, vertexCount);

    const matrix = new THREE.Matrix4();
    const vertexColor = new THREE.Color();

    for (let i = 0; i < vertexCount; i++) {
        matrix.makeTranslation(positions[i * 3], positions[i * 3 + 1], positions[i * 3 + 2]);
        instancedMesh.setMatrixAt(i, matrix);

        vertexColor.setRGB(colors[i * 3], colors[i * 3 + 1], colors[i * 3 + 2]);
        instancedMesh.setColorAt(i, vertexColor);
    }

    instancedMesh.visible = false;
    return instancedMesh;
}

export function buildHexMesh(data: MeshData): HexMeshGroup {
    const group = new THREE.Group();

    const primalMesh = buildPrimalMesh(data);
    const primalWireMesh = buildPrimalWire(data);
    const dualMesh = buildDualMesh(data);
    const dualWireMesh = buildDualWire(data);
    const anchorWireMesh = buildAnchorWire(data);
    const anchorVertexMesh = buildAnchorVertices(data);

    group.add(primalMesh);
    group.add(primalWireMesh);
    group.add(dualMesh);
    group.add(dualWireMesh);
    group.add(anchorWireMesh);
    group.add(anchorVertexMesh);

    const dispose = () => {
        disposeMesh(primalMesh);
        disposeMesh(primalWireMesh);
        disposeMesh(dualMesh);
        disposeMesh(dualWireMesh);
        disposeMesh(anchorWireMesh);
        disposeMesh(anchorVertexMesh);
    };

    return {
        group,
        setPrimalMeshVisible: (visible: boolean) => {
            primalMesh.visible = visible;
        },
        setPrimalWireVisible: (visible: boolean) => {
            primalWireMesh.visible = visible;
        },
        setDualMeshVisible: (visible: boolean) => {
            dualMesh.visible = visible;
        },
        setDualWireVisible: (visible: boolean) => {
            dualWireMesh.visible = visible;
        },
        setAnchorVisible: (visible: boolean) => {
            anchorWireMesh.visible = visible;
        },
        setAnchorVerticesVisible: (visible: boolean) => {
            anchorVertexMesh.visible = visible;
        },
        dispose
    };
}
