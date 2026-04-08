import * as THREE from 'three';
import { PolygonData } from './geometry-data';

/**
 * Manages wireframe visualization of polygons.
 * Creates thick lines showing all polygon boundaries.
 */
export class PolygonWireMesh {
    private mesh: THREE.LineSegments | null = null;

    constructor(
        private readonly parent: THREE.Group,
        private readonly polygonData: PolygonData
    ) {}

    /**
     * Show the wireframe mesh for all polygons.
     */
    show(): void {
        if (this.mesh) return; // Already visible

        const geometry = this.buildGeometry();
        if (!geometry.attributes.position || geometry.attributes.position.count === 0) {
            geometry.dispose();
            return;
        }

        const material = new THREE.LineBasicMaterial({
            color: 0x00ffff,
            linewidth: 2
        });
        this.mesh = new THREE.LineSegments(geometry, material);
        this.mesh.renderOrder = 1; // Render on top of base mesh
        this.parent.add(this.mesh);
    }

    /**
     * Hide the wireframe mesh.
     */
    hide(): void {
        if (!this.mesh) return;

        this.parent.remove(this.mesh);
        this.mesh.geometry.dispose();
        (this.mesh.material as THREE.Material).dispose();
        this.mesh = null;
    }

    /**
     * Build geometry containing all polygon edges as line segments.
     */
    private buildGeometry(): THREE.BufferGeometry {
        const positions: number[] = [];

        const vertexCount = this.polygonData.starts.length - 1;

        for (let vi = 0; vi < vertexCount; vi++) {
            const start = this.polygonData.starts[vi];
            const end = this.polygonData.starts[vi + 1];

            if (end <= start) {
                // No polygon for this vertex
                continue;
            }

            const polygonIndices = this.polygonData.indices.slice(start, end);
            const n = polygonIndices.length;

            // Create line segments for each edge of the polygon
            for (let i = 0; i < n; i++) {
                const next = (i + 1) % n;
                const idx0 = polygonIndices[i];
                const idx1 = polygonIndices[next];

                const x0 = this.polygonData.vertices[idx0 * 2];
                const y0 = this.polygonData.vertices[idx0 * 2 + 1];
                const x1 = this.polygonData.vertices[idx1 * 2];
                const y1 = this.polygonData.vertices[idx1 * 2 + 1];

                // Add line segment (each edge as a pair of vertices)
                positions.push(x0, y0, 0, x1, y1, 0);
            }
        }

        const geometry = new THREE.BufferGeometry();
        if (positions.length > 0) {
            geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
        }

        return geometry;
    }

    get isVisible(): boolean {
        return this.mesh !== null;
    }

    dispose(): void {
        this.hide();
    }
}
