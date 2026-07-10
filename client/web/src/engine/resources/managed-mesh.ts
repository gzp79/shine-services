import * as THREE from 'three';
import {
    type OwnedBufferGeometry,
    type OwnedMaterial,
    type SharedBufferGeometry,
    type SharedMaterial,
    disposeIfOwned,
    own
} from './ownership';

/** A Mesh that disposes owned geometry/material on dispose(); skips shared ones. */
export class ManagedMesh extends THREE.Mesh {
    constructor(geometry: OwnedBufferGeometry | SharedBufferGeometry, material: OwnedMaterial | SharedMaterial) {
        super(geometry, material);
    }

    static own(geometry: THREE.BufferGeometry, material: THREE.Material): ManagedMesh {
        return new ManagedMesh(own(geometry), own(material));
    }

    dispose(): void {
        disposeIfOwned(this.geometry as OwnedBufferGeometry | SharedBufferGeometry);
        disposeIfOwned(this.material as OwnedMaterial | SharedMaterial);
    }
}

/** A LineSegments that disposes owned geometry/material on dispose(); skips shared ones. */
export class ManagedLineSegments extends THREE.LineSegments {
    constructor(geometry: OwnedBufferGeometry | SharedBufferGeometry, material: OwnedMaterial | SharedMaterial) {
        super(geometry, material);
    }

    static own(geometry: THREE.BufferGeometry, material: THREE.Material): ManagedLineSegments {
        return new ManagedLineSegments(own(geometry), own(material));
    }

    dispose(): void {
        disposeIfOwned(this.geometry as OwnedBufferGeometry | SharedBufferGeometry);
        disposeIfOwned(this.material as OwnedMaterial | SharedMaterial);
    }
}
