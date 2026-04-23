import * as THREE from 'three';
import { disposeMesh } from '../experiment';

const EDGE_COLOR = 0x222222;
const DUAL_EDGE_COLOR = 0x222222;

export interface MeshData {
    // Primal mesh (quads)
    vertices: Float32Array; // [x0, y0, x1, y1, ...]
    quad_indices: Uint32Array; // Quad indices
    quad_ranges: Uint32Array; // [start0, end0, start1, end1, ...]
    // Anchor edges (boundary wires from primal mesh)
    anchor_indices: Uint32Array; // Wire indices
    anchor_ranges: Uint32Array; // [start0, end0, start1, end1, ...]
    // Dual mesh (dual polygons)
    dual_vertices: Float32Array; // [x0, y0, x1, y1, ...]
    dual_indices: Uint32Array; // Polygon indices
    dual_ranges: Uint32Array; // [start0, end0, start1, end1, ...]
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
    const primalPositions: number[] = [];
    const primalColors: number[] = [];
    const color = new THREE.Color();

    for (let q = 0; q < data.quad_ranges.length; q += 2) {
        const start = data.quad_ranges[q];
        const end = data.quad_ranges[q + 1];

        if (end - start !== 4) continue; // Skip non-quads

        const a = data.quad_indices[start];
        const b = data.quad_indices[start + 1];
        const c = data.quad_indices[start + 2];
        const d = data.quad_indices[start + 3];

        // Generate a random color for this quad using HSL for better distribution
        const hue = ((q / 2) * 0.618033988749895) % 1.0; // Golden ratio for nice distribution
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
    const positions: number[] = [];

    for (let q = 0; q < data.quad_ranges.length; q += 2) {
        const start = data.quad_ranges[q];
        const end = data.quad_ranges[q + 1];
        const quadSize = end - start;

        for (let e = 0; e < quadSize; e++) {
            const i0 = data.quad_indices[start + e];
            const i1 = data.quad_indices[start + ((e + 1) % quadSize)];
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
    const positions: number[] = [];
    const colors: number[] = [];
    const color = new THREE.Color();

    for (let p = 0; p < data.dual_ranges.length; p += 2) {
        const start = data.dual_ranges[p];
        const end = data.dual_ranges[p + 1];
        const polySize = end - start;

        if (polySize < 3) continue; // Skip degenerate polygons

        // Generate color for this dual polygon
        const hue = ((p / 2) * 0.618033988749895) % 1.0;
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
    const positions: number[] = [];

    for (let p = 0; p < data.dual_ranges.length; p += 2) {
        const start = data.dual_ranges[p];
        const end = data.dual_ranges[p + 1];
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
    const positions: number[] = [];
    const colors: number[] = [];
    const color = new THREE.Color();

    const anchorEdgeCount = data.anchor_ranges.length / 2;

    for (let a = 0; a < data.anchor_ranges.length; a += 2) {
        const start = data.anchor_ranges[a];
        const end = data.anchor_ranges[a + 1];
        const edgeSize = end - start;

        if (edgeSize < 2) continue;

        // Use HSL color wheel for distinct colors per anchor edge
        const hue = (a / 2 / anchorEdgeCount) % 1.0;
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
    const positions: number[] = [];
    const colors: number[] = [];
    const vertexSet = new Set<number>();
    const color = new THREE.Color();

    const anchorEdgeCount = data.anchor_ranges.length / 2;

    for (let a = 0; a < data.anchor_ranges.length; a += 2) {
        const start = data.anchor_ranges[a];
        const end = data.anchor_ranges[a + 1];

        const hue = (a / 2 / anchorEdgeCount) % 1.0;
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
