import * as THREE from 'three';
import {
    ZOOM_DISTANCE_SCALE,
    MAX_CAMERA_DISTANCE,
    MAX_CAMERA_PITCH,
    MIN_CAMERA_DISTANCE,
    MIN_CAMERA_PITCH
} from '../constants';
import { EventDispatcher, EventSubscriptions } from '../engine/events';
import type { RenderContext } from '../engine/render-context';
import { WORLD_REFERENCE_CHANGED, type WorldReferenceChangedEvent } from '../systems/world-reference-system';

export const INPUT_CONTROLLER_CHANGED = 'input:controller:changed';
export type InputControllerChangedEvent = { controller: string };

export const CURSOR_INTERACT_START = 'cursor:interact_start';
export const CURSOR_INTERACT = 'cursor:interact';
export const CURSOR_INTERACT_END = 'cursor:interact_end';
export type CursorInteractEvent = { pos: THREE.Vector3 };

export type CursorInteractKind = 'start' | 'move' | 'end';

export class WorldCursor {
    readonly position = new THREE.Vector3(0, 0, 0);
    private readonly direction = new THREE.Vector3(0, 1, 0);
    cameraDistance = 600;
    cameraYaw = 0;
    private mesh: THREE.Mesh | null = null;
    private readonly subscriptions: EventSubscriptions;
    private readonly dispatcher: EventDispatcher;

    constructor(
        private readonly renderContext: RenderContext,
        events: EventTarget
    ) {
        this.subscriptions = new EventSubscriptions(events);
        this.dispatcher = new EventDispatcher(events);
        this.subscriptions.on<WorldReferenceChangedEvent>(WORLD_REFERENCE_CHANGED, this.handleWorldReferenceChanged);
    }

    get showMesh(): boolean {
        return this.mesh?.visible ?? false;
    }

    set showMesh(value: boolean) {
        if (value) {
            this.createMesh();
        } else {
            this.disposeMesh();
        }
    }

    dispose(): void {
        this.subscriptions.dispose();
        this.disposeMesh();
    }

    getCameraTarget(): {
        distance: number;
        yaw: number;
        cursorPosition: THREE.Vector3;
        position: THREE.Vector3;
        lookAt: THREE.Vector3;
    } {
        const t = (this.cameraDistance - MIN_CAMERA_DISTANCE) / (MAX_CAMERA_DISTANCE - MIN_CAMERA_DISTANCE);
        const pitch = MIN_CAMERA_PITCH + t * (MAX_CAMERA_PITCH - MIN_CAMERA_PITCH);

        const height = this.cameraDistance * Math.sin(pitch);
        const horizontalDist = this.cameraDistance * Math.cos(pitch);

        const position = new THREE.Vector3(
            this.position.x - horizontalDist * Math.sin(this.cameraYaw),
            this.position.y - horizontalDist * Math.cos(this.cameraYaw),
            this.position.z + height
        );

        const lookAt = this.position.clone().add(this.direction.clone().multiplyScalar(10));

        return {
            distance: this.cameraDistance,
            yaw: this.cameraYaw,
            cursorPosition: this.position.clone(),
            position,
            lookAt
        };
    }

    setPosition(pos: { x: number; y: number; z: number }): void {
        this.position.set(pos.x, pos.y, pos.z);
        if (this.mesh) {
            this.mesh.position.copy(this.position);
        }
    }

    moveBy(delta: THREE.Vector3): void {
        this.setPosition(this.position.clone().add(delta));
    }

    rotateBy(angleDelta: number): void {
        this.setYaw(this.cameraYaw - angleDelta);
    }

    zoomBy(delta: number): void {
        this.setZoom(this.cameraDistance + delta);
    }

    zoomByDelta(delta: number): void {
        const midDistance = (MAX_CAMERA_DISTANCE + MIN_CAMERA_DISTANCE) / 2;
        const zoomScale = (this.cameraDistance / midDistance) * ZOOM_DISTANCE_SCALE;
        this.setZoom(this.cameraDistance + delta * zoomScale);
    }

    dispatchSchemaChanged(schema: string): void {
        this.dispatcher.dispatch<InputControllerChangedEvent>(INPUT_CONTROLLER_CHANGED, { controller: schema });
    }

    dispatchInteract(kind: CursorInteractKind, worldPos: THREE.Vector3): void {
        const eventName =
            kind === 'start' ? CURSOR_INTERACT_START :
            kind === 'end'   ? CURSOR_INTERACT_END :
                               CURSOR_INTERACT;
        this.dispatcher.dispatch<CursorInteractEvent>(eventName, { pos: worldPos });
    }

    private setYaw(yaw: number): void {
        const TWO_PI = 2 * Math.PI;
        this.cameraYaw = ((yaw % TWO_PI) + TWO_PI) % TWO_PI;
        this.direction.set(Math.sin(this.cameraYaw), Math.cos(this.cameraYaw), 0);
        if (this.mesh) {
            this.mesh.rotation.z = Math.atan2(this.direction.x, this.direction.y);
        }
    }

    private setZoom(distance: number): void {
        this.cameraDistance = Math.max(MIN_CAMERA_DISTANCE, Math.min(MAX_CAMERA_DISTANCE, distance));
    }

    private handleWorldReferenceChanged = (event: WorldReferenceChangedEvent): void => {
        this.position.x += event.deltaPosition.x;
        this.position.y += event.deltaPosition.y;
    };

    private createMesh(): void {
        if (this.mesh) return;

        const geometry = new THREE.BufferGeometry();
        const headLength = 30;
        const baseWidth = 15;
        const vertices = new Float32Array([
            0, headLength, 0,
            -baseWidth, -5, 0,
            baseWidth, -5, 0
        ]);
        geometry.setAttribute('position', new THREE.BufferAttribute(vertices, 3));
        geometry.setIndex([0, 1, 2]);

        const material = new THREE.MeshBasicMaterial({
            color: 0x0000ff,
            side: THREE.DoubleSide,
            depthTest: false,
            depthWrite: false
        });
        this.mesh = new THREE.Mesh(geometry, material);
        this.mesh.renderOrder = 998;
        this.mesh.frustumCulled = false;
        this.mesh.position.copy(this.position);

        if (this.direction.lengthSq() > 0) {
            this.mesh.rotation.z = Math.atan2(this.direction.x, this.direction.y);
        }

        this.renderContext.scene.add(this.mesh);
    }

    private disposeMesh(): void {
        if (!this.mesh) return;

        this.mesh.parent?.remove(this.mesh);
        this.mesh.geometry.dispose();
        (this.mesh.material as THREE.Material).dispose();
        this.mesh = null;
    }
}
