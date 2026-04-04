import * as THREE from 'three';
import { WORLD_REFERENCE_CHANGED, type WorldReferenceChangedEvent } from '../../systems/world-reference-system';
import { EventSubscriptions } from '../events';
import type { RenderContext } from '../render-context';
import { VIEWPORT_RESIZE, type ViewportResizeEvent } from '../render-context';

export class Camera {
    readonly camera: THREE.PerspectiveCamera;
    private readonly subscriptions: EventSubscriptions;

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

    private handleViewportResize = (event: ViewportResizeEvent): void => {
        this.camera.aspect = event.width / event.height;
        this.camera.updateProjectionMatrix();
    };

    private handleWorldReferenceChanged = (event: WorldReferenceChangedEvent): void => {
        this.camera.position.x += event.deltaPosition.x;
        this.camera.position.y += event.deltaPosition.y;
    };
}
