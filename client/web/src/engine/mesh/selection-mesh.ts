import * as THREE from 'three';
import { PolygonData } from './geometry-data';
import { buildPrismGeometry } from './prism-geometry';
import { createWireframeGlowMaterial, createWireframeMaterial } from './wireframe-shader';

/**
 * Manages a single selection mesh for hover interaction.
 * Creates and disposes prism geometry on demand.
 * Renders with two passes: sharp core + glow halo.
 */
export class SelectionMesh {
    private meshCore: THREE.Mesh | null = null;
    private meshGlow: THREE.Mesh | null = null;
    private currentVertIdx: number = -1;

    constructor(
        private readonly parent: THREE.Group,
        private readonly polygonData: PolygonData
    ) {}

    /**
     * Show selection at the given vertex index.
     * If already showing this vertex, does nothing.
     * Returns true if mesh was created/updated.
     */
    showAt(vertIdx: number): boolean {
        // Already showing this vertex
        if (this.currentVertIdx === vertIdx && this.meshCore) {
            return false;
        }

        // Hide previous
        this.hide();

        // Check if vertex has a polygon
        const start = this.polygonData.starts[vertIdx];
        const end = this.polygonData.starts[vertIdx + 1];
        if (end <= start) return false; // No polygon for this vertex

        // Build geometry
        const polygonIndices = this.polygonData.indices.slice(start, end);
        const geometry = buildPrismGeometry(polygonIndices, this.polygonData.vertices);

        if (!geometry.attributes.position || geometry.attributes.position.count === 0) {
            geometry.dispose();
            return false;
        }

        // Create glow halo (rendered first, behind sharp core)
        const glowMaterial = createWireframeGlowMaterial(0xffdd00, 0.5);
        this.meshGlow = new THREE.Mesh(geometry, glowMaterial);
        this.meshGlow.userData = { vertIdx };
        this.meshGlow.renderOrder = -1; // Render behind core
        this.parent.add(this.meshGlow);

        // Create sharp core (rendered second, in front)
        const coreMaterial = createWireframeMaterial(0xffdd00);
        this.meshCore = new THREE.Mesh(geometry, coreMaterial);
        this.meshCore.userData = { vertIdx };
        this.meshCore.renderOrder = 0; // Render in front of glow
        this.parent.add(this.meshCore);

        this.currentVertIdx = vertIdx;

        return true;
    }

    hide(): void {
        if (!this.meshCore && !this.meshGlow) return;

        if (this.meshGlow) {
            this.parent.remove(this.meshGlow);
            (this.meshGlow.material as THREE.Material).dispose();
            this.meshGlow = null;
        }

        if (this.meshCore) {
            this.parent.remove(this.meshCore);
            this.meshCore.geometry.dispose();
            (this.meshCore.material as THREE.Material).dispose();
            this.meshCore = null;
        }

        this.currentVertIdx = -1;
    }

    get vertIdx(): number {
        return this.currentVertIdx;
    }

    get isVisible(): boolean {
        return this.meshCore !== null;
    }

    dispose(): void {
        this.hide();
    }
}
