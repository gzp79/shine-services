import * as THREE from 'three';
import { buildGeometryFromPolygons, buildGeometryFromWires } from '../mesh/builder';
import type { PolygonMesh, WiredPolygonMesh } from '../mesh/polygon-mesh';

export class WireNode {
    private mesh: THREE.LineSegments | null = null;

    private constructor(
        private readonly parent: THREE.Group,
        private readonly buildGeometryFn: () => THREE.BufferGeometry
    ) {}

    static fromPolygons(parent: THREE.Group, mesh: PolygonMesh): WireNode {
        return new WireNode(parent, () => buildGeometryFromPolygons(mesh));
    }

    static fromWires(parent: THREE.Group, mesh: WiredPolygonMesh): WireNode {
        return new WireNode(parent, () => buildGeometryFromWires(mesh));
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
