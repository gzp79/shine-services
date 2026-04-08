import * as THREE from 'three';
import { WORLD_REFERENCE_CHANGED, type WorldReferenceChangedEvent } from '../../systems/world-reference-system';
import { EventSubscriptions } from '../events';
import type { RenderContext } from '../render-context';
import { VIEWPORT_RESIZE, type ViewportResizeEvent } from '../render-context';

export class Camera {
    readonly camera: THREE.PerspectiveCamera;
    private readonly subscriptions: EventSubscriptions;
    private readonly raycaster = new THREE.Raycaster();

    constructor(renderContext: RenderContext, events: EventTarget) {
        this.subscriptions = new EventSubscriptions(events);

        // Create camera
        const aspect = renderContext.width / renderContext.height;
        this.camera = new THREE.PerspectiveCamera(50, aspect, 1, 50000);
        this.camera.up.set(0, 0, 1);
        this.camera.position.set(0, -1200, 2000);
        this.camera.lookAt(0, 0, 0);

        this.subscriptions.on<ViewportResizeEvent>(VIEWPORT_RESIZE, this.handleViewportResize);
        this.subscriptions.on<WorldReferenceChangedEvent>(WORLD_REFERENCE_CHANGED, this.handleWorldReferenceChanged);
    }

    update(): void {}

    dispose(): void {
        this.subscriptions.dispose();
    }

    screenToWorldPlanePoint(screenX: number, screenY: number, planeZ = 0): THREE.Vector3 | null {
        const ndcX = (screenX / window.innerWidth) * 2 - 1;
        const ndcY = -(screenY / window.innerHeight) * 2 + 1;
        return this.ndcToWorldPlanePoint(ndcX, ndcY, planeZ);
    }

    ndcToWorldPlanePoint(ndcX: number, ndcY: number, planeZ = 0): THREE.Vector3 | null {
        this.raycaster.setFromCamera(new THREE.Vector2(ndcX, ndcY), this.camera);

        // Intersect with a plane at y=0 (ground plane)
        const plane = new THREE.Plane(new THREE.Vector3(0, 0, 1), -planeZ);
        const intersectionPoint = new THREE.Vector3();
        this.raycaster.ray.intersectPlane(plane, intersectionPoint);

        return intersectionPoint;
    }

    private handleViewportResize = (event: ViewportResizeEvent): void => {
        this.camera.aspect = event.width / event.height;
        this.camera.updateProjectionMatrix();
    };

    private handleWorldReferenceChanged = (event: WorldReferenceChangedEvent): void => {
        this.camera.position.x += event.deltaPosition.x;
        this.camera.position.y += event.deltaPosition.y;
    };
}
