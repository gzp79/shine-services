import * as THREE from 'three';
import type { CdtMeshHandle } from '../../wasm-types/shine_game';

const FILL_COLOR = new THREE.Color(0.82, 0.85, 0.88);
const EDGE_COLOR = 0x555555;
const FIXED_EDGE_COLOR = 0xff4444;
const CIRCLE_COLOR = 0x44aa44;
const POINT_COLOR = 0x2288aa;
const ACTIVE_TRI_COLOR = 0x3366ff;

export interface CdtMeshGroup {
    group: THREE.Group;
    dispose: () => void;
}

function circumcircle(
    a: { x: number; y: number },
    b: { x: number; y: number },
    c: { x: number; y: number }
): { x: number; y: number; radius: number } | null {
    const d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if (Math.abs(d) < 1e-10) { return null; }

    const a2 = a.x * a.x + a.y * a.y;
    const b2 = b.x * b.x + b.y * b.y;
    const c2 = c.x * c.x + c.y * c.y;

    const ux = (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d;
    const uy = (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d;
    const radius = Math.sqrt((ux - a.x) ** 2 + (uy - a.y) ** 2);

    return { x: ux, y: uy, radius };
}

export function buildCircumcenterMesh(cdt: CdtMeshHandle, tri: number): CdtMeshGroup | null {
    const vertices = cdt.vertices();
    const triangles = cdt.triangles();

    const triCount = triangles.length / 3;
    if (tri < 0 || tri >= triCount) { return null; }

    const group = new THREE.Group();

    const ia = triangles[tri * 3];
    const ib = triangles[tri * 3 + 1];
    const ic = triangles[tri * 3 + 2];

    const a = { x: vertices[ia * 2], y: vertices[ia * 2 + 1] };
    const b = { x: vertices[ib * 2], y: vertices[ib * 2 + 1] };
    const c = { x: vertices[ic * 2], y: vertices[ic * 2 + 1] };

    const cc = circumcircle(a, b, c);
    if (cc !== null) {
        const { x: cx, y: cy, radius } = cc;

        const circlePositions: number[] = [];
        const segments = 64;
        for (let s = 0; s < segments; s++) {
            const angle1 = (s / segments) * Math.PI * 2;
            const angle2 = ((s + 1) / segments) * Math.PI * 2;
            circlePositions.push(
                cx + radius * Math.cos(angle1), cy + radius * Math.sin(angle1), 20,
                cx + radius * Math.cos(angle2), cy + radius * Math.sin(angle2), 20
            );
        }

        const pointGeom = new THREE.BufferGeometry();
        pointGeom.setAttribute('position', new THREE.Float32BufferAttribute([cx, cy, 25], 3));
        group.add(new THREE.Points(pointGeom, new THREE.PointsMaterial({ color: CIRCLE_COLOR, size: 60, sizeAttenuation: true })));

        const triGeom = new THREE.BufferGeometry();
        triGeom.setAttribute('position', new THREE.Float32BufferAttribute([a.x, a.y, 15, b.x, b.y, 15, c.x, c.y, 15], 3));
        group.add(new THREE.Mesh(triGeom, new THREE.MeshBasicMaterial({ color: ACTIVE_TRI_COLOR, side: THREE.DoubleSide, transparent: true, opacity: 0.4 })));

        const triEdgeGeom = new THREE.BufferGeometry();
        triEdgeGeom.setAttribute('position', new THREE.Float32BufferAttribute([a.x, a.y, 16, b.x, b.y, 16, b.x, b.y, 16, c.x, c.y, 16, c.x, c.y, 16, a.x, a.y, 16], 3));
        group.add(new THREE.LineSegments(triEdgeGeom, new THREE.LineBasicMaterial({ color: ACTIVE_TRI_COLOR, linewidth: 3 })));

        const circleGeom = new THREE.BufferGeometry();
        circleGeom.setAttribute('position', new THREE.Float32BufferAttribute(circlePositions, 3));
        group.add(new THREE.LineSegments(circleGeom, new THREE.LineBasicMaterial({ color: CIRCLE_COLOR, linewidth: 2 })));
    }

    const dispose = () => {
        group.traverse((obj) => {
            if (obj instanceof THREE.Mesh || obj instanceof THREE.LineSegments || obj instanceof THREE.Points) {
                obj.geometry.dispose();
                if (obj.material instanceof THREE.Material) { obj.material.dispose(); }
            }
        });
    };

    return { group, dispose };
}

export function buildCdtMesh(cdt: CdtMeshHandle): CdtMeshGroup {
    const vertices = cdt.vertices();
    const triangles = cdt.triangles();
    const constraints = cdt.constraints();

    const group = new THREE.Group();
    const vertCount = vertices.length / 2;
    const triCount = triangles.length / 3;

    const positions = new Float32Array(vertCount * 3);
    for (let i = 0; i < vertCount; i++) {
        positions[i * 3] = vertices[i * 2];
        positions[i * 3 + 1] = vertices[i * 2 + 1];
        positions[i * 3 + 2] = 0;
    }

    // Filled triangles
    const triPositions: number[] = [];
    const triColors: number[] = [];
    for (let t = 0; t < triCount; t++) {
        const a = triangles[t * 3];
        const b = triangles[t * 3 + 1];
        const c = triangles[t * 3 + 2];
        for (const idx of [a, b, c]) {
            triPositions.push(positions[idx * 3], positions[idx * 3 + 1], positions[idx * 3 + 2]);
            triColors.push(FILL_COLOR.r, FILL_COLOR.g, FILL_COLOR.b);
        }
    }
    const fillGeom = new THREE.BufferGeometry();
    fillGeom.setAttribute('position', new THREE.Float32BufferAttribute(triPositions, 3));
    fillGeom.setAttribute('color', new THREE.Float32BufferAttribute(triColors, 3));
    group.add(new THREE.Mesh(fillGeom, new THREE.MeshBasicMaterial({ vertexColors: true, side: THREE.DoubleSide })));

    // Triangle edge wireframe
    const edgePositions: number[] = [];
    for (let t = 0; t < triCount; t++) {
        const base = t * 3;
        for (let e = 0; e < 3; e++) {
            const i0 = triangles[base + e];
            const i1 = triangles[base + ((e + 1) % 3)];
            edgePositions.push(positions[i0 * 3], positions[i0 * 3 + 1], 10, positions[i1 * 3], positions[i1 * 3 + 1], 10);
        }
    }
    const edgeGeom = new THREE.BufferGeometry();
    edgeGeom.setAttribute('position', new THREE.Float32BufferAttribute(edgePositions, 3));
    group.add(new THREE.LineSegments(edgeGeom, new THREE.LineBasicMaterial({ color: EDGE_COLOR })));

    // Constraint edges
    const fixedCount = constraints.length / 2;
    if (fixedCount > 0) {
        const fixedPositions: number[] = [];
        for (let e = 0; e < fixedCount; e++) {
            const i0 = constraints[e * 2];
            const i1 = constraints[e * 2 + 1];
            fixedPositions.push(positions[i0 * 3], positions[i0 * 3 + 1], 10, positions[i1 * 3], positions[i1 * 3 + 1], 10);
        }
        const fixedGeom = new THREE.BufferGeometry();
        fixedGeom.setAttribute('position', new THREE.Float32BufferAttribute(fixedPositions, 3));
        group.add(new THREE.LineSegments(fixedGeom, new THREE.LineBasicMaterial({ color: FIXED_EDGE_COLOR, linewidth: 2 })));
    }

    // Point sprites
    const pointPositions = Array.from({ length: vertCount }, (_, i) => [positions[i * 3], positions[i * 3 + 1], 15]).flat();
    const pointGeom = new THREE.BufferGeometry();
    pointGeom.setAttribute('position', new THREE.Float32BufferAttribute(pointPositions, 3));
    group.add(new THREE.Points(pointGeom, new THREE.PointsMaterial({ color: POINT_COLOR, size: 40, sizeAttenuation: true })));

    const dispose = () => {
        group.traverse((obj) => {
            if (obj instanceof THREE.Mesh || obj instanceof THREE.LineSegments || obj instanceof THREE.Points) {
                obj.geometry.dispose();
                if (obj.material instanceof THREE.Material) { obj.material.dispose(); }
            }
        });
    };

    return { group, dispose };
}
