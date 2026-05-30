import * as THREE from 'three';
import { WORLD_REFERENCE_CHANGED, type WorldReferenceChangedEvent } from '../../systems/world-reference-system';
import { EventSubscriptions } from '../events';

export interface ICamera {
    readonly width: number;
    readonly height: number;
    readonly aspect: number;
    screenToWorldPlanePoint(screenX: number, screenY: number, planeZ?: number): THREE.Vector3 | null;
    ndcToWorldPlanePoint(ndcX: number, ndcY: number, planeZ?: number): THREE.Vector3 | null;
}

export class Camera implements ICamera {
    readonly camera: THREE.PerspectiveCamera;
    private readonly subscriptions: EventSubscriptions;
    private readonly raycaster = new THREE.Raycaster();
    width = 1;
    height = 1;

    get aspect(): number {
        return this.camera.aspect;
    }

    constructor(events: EventTarget) {
        this.subscriptions = new EventSubscriptions(events);

        this.camera = new THREE.PerspectiveCamera(50, 1, 1, 50000);
        this.camera.up.set(0, 0, 1);
        this.camera.position.set(0, -1200, 2000);
        this.camera.lookAt(0, 0, 0);

        this.subscriptions.on<WorldReferenceChangedEvent>(WORLD_REFERENCE_CHANGED, this.handleWorldReferenceChanged);
    }

    dispose(): void {
        this.subscriptions.dispose();
    }

    screenToWorldPlanePoint(screenX: number, screenY: number, planeZ = 0): THREE.Vector3 | null {
        const ndcX = (screenX / this.width) * 2 - 1;
        const ndcY = -(screenY / this.height) * 2 + 1;
        return this.ndcToWorldPlanePoint(ndcX, ndcY, planeZ);
    }

    ndcToWorldPlanePoint(ndcX: number, ndcY: number, planeZ = 0): THREE.Vector3 | null {
        this.raycaster.setFromCamera(new THREE.Vector2(ndcX, ndcY), this.camera);
        const plane = new THREE.Plane(new THREE.Vector3(0, 0, 1), -planeZ);
        const intersectionPoint = new THREE.Vector3();
        return this.raycaster.ray.intersectPlane(plane, intersectionPoint);
    }

    private handleWorldReferenceChanged = (event: WorldReferenceChangedEvent): void => {
        this.camera.position.x += event.deltaPosition.x;
        this.camera.position.y += event.deltaPosition.y;
    };
}
