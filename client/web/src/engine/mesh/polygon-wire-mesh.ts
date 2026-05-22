import * as THREE from 'three';
import type { PolygonMesh, WiredPolygonMesh } from './polygon-mesh';

/**
 * Manages wireframe visualization of polygons.
 * Creates thick lines showing all polygon boundaries.
 */
export class PolygonWireMesh {
    private mesh: THREE.LineSegments | null = null;

    private constructor(
        private readonly parent: THREE.Group,
        private readonly buildGeometryFn: () => THREE.BufferGeometry
    ) {}

    static fromPolygons(parent: THREE.Group, mesh: PolygonMesh): PolygonWireMesh {
        return new PolygonWireMesh(parent, () => buildGeometryFromPolygons(mesh));
    }

    static fromWires(parent: THREE.Group, mesh: WiredPolygonMesh): PolygonWireMesh {
        return new PolygonWireMesh(parent, () => buildGeometryFromWires(mesh));
    }

    show(): void {
        if (this.mesh) return;

        const geometry = this.buildGeometryFn();
        if (!geometry.attributes.position || geometry.attributes.position.count === 0) {
            geometry.dispose();
            return;
        }

        const material = new THREE.LineBasicMaterial({ color: 0x00ffff, linewidth: 2 });
        this.mesh = new THREE.LineSegments(geometry, material);
        this.mesh.renderOrder = 1;
        this.parent.add(this.mesh);
    }

    hide(): void {
        if (!this.mesh) return;
        this.parent.remove(this.mesh);
        this.mesh.geometry.dispose();
        (this.mesh.material as THREE.Material).dispose();
        this.mesh = null;
    }

    isVisible(): boolean {
        return this.mesh !== null;
    }

    dispose(): void {
        this.hide();
    }
}

function buildGeometryFromPolygons(mesh: PolygonMesh): THREE.BufferGeometry {
    const positions: number[] = [];
    const { vertices, indices, ranges } = mesh;
    const polygonCount = ranges.length / 2;

    for (let p = 0; p < polygonCount; p++) {
        const start = ranges[p * 2];
        const end = ranges[p * 2 + 1];
        if (end <= start) continue;

        const n = end - start;
        for (let i = 0; i < n; i++) {
            const idx0 = indices[start + i];
            const idx1 = indices[start + ((i + 1) % n)];
            positions.push(
                vertices[idx0 * 2],
                vertices[idx0 * 2 + 1],
                0,
                vertices[idx1 * 2],
                vertices[idx1 * 2 + 1],
                0
            );
        }
    }

    const geometry = new THREE.BufferGeometry();
    if (positions.length > 0) {
        geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
    }
    return geometry;
}

function buildGeometryFromWires(mesh: WiredPolygonMesh): THREE.BufferGeometry {
    const positions: number[] = [];
    const { vertices, wireIndices, wireRanges } = mesh;
    const wireCount = wireRanges.length / 2;

    for (let w = 0; w < wireCount; w++) {
        const start = wireRanges[w * 2];
        const end = wireRanges[w * 2 + 1];
        if (end <= start) continue;

        for (let i = start; i + 1 < end; i++) {
            const idx0 = wireIndices[i];
            const idx1 = wireIndices[i + 1];
            positions.push(
                vertices[idx0 * 2],
                vertices[idx0 * 2 + 1],
                0,
                vertices[idx1 * 2],
                vertices[idx1 * 2 + 1],
                0
            );
        }
    }

    const geometry = new THREE.BufferGeometry();
    if (positions.length > 0) {
        geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
    }
    return geometry;
}
