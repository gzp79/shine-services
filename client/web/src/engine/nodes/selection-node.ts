import * as THREE from 'three';
import { buildPrismGeometry } from '../mesh/builder';
import type { PolygonMesh } from '../mesh/polygon-mesh';

export class SelectionNode {
    private mesh: THREE.Mesh | null = null;

    constructor(
        private readonly parent: THREE.Group,
        private readonly polygonData: PolygonMesh
    ) {}

    show(polygonId: number): void {
        this.hide();

        const geometry = buildPrismGeometry(this.polygonData, polygonId);
        if (!geometry.attributes.position || geometry.attributes.position.count === 0) {
            geometry.dispose();
            return;
        }

        const material = new THREE.MeshBasicMaterial({
            color: 0xffdd00,
            transparent: true,
            opacity: 0.5,
            side: THREE.DoubleSide,
            depthWrite: false
        });
        this.mesh = new THREE.Mesh(geometry, material);
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
