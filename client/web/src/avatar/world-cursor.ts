import * as THREE from 'three';
import {
    CURSOR_MOVE_SPEED,
    CURSOR_SPRINT_MULTIPLIER,
    MAX_CAMERA_DISTANCE,
    MAX_CAMERA_PITCH,
    MIN_CAMERA_DISTANCE,
    MIN_CAMERA_PITCH,
    ZOOM_DISTANCE_SCALE
} from '../constants';
import { EventSubscriptions } from '../engine/events';
import type { RenderContext } from '../engine/render-context';
import { WORLD_REFERENCE_CHANGED, type WorldReferenceChangedEvent } from '../systems/world-reference-system';
import {
    CURSOR_MOVE,
    CURSOR_MOVE_TO,
    CURSOR_ROTATE,
    CURSOR_ROTATE_DELTA,
    CURSOR_ZOOM,
    CURSOR_ZOOM_DELTA,
    type CursorMoveEvent,
    type CursorMoveToEvent,
    type CursorRotateDeltaEvent,
    type CursorRotateEvent,
    type CursorZoomDeltaEvent,
    type CursorZoomEvent
} from './events';

export class WorldCursor {
    readonly position = new THREE.Vector3(0, 0, 0);
    private readonly direction = new THREE.Vector3(0, 1, 0);
    private readonly velocity = new THREE.Vector3(0, 0, 0);
    private cameraDistance = 600;
    private cameraYaw = 0;
    private rotateRate = 0;
    private zoomRate = 0;
    private mesh: THREE.Mesh | null = null;
    private readonly subscriptions: EventSubscriptions;

    constructor(
        private readonly renderContext: RenderContext,
        events: EventTarget
    ) {
        this.subscriptions = new EventSubscriptions(events);

        this.subscriptions.on<CursorMoveEvent>(CURSOR_MOVE, this.handleCursorMove);
        this.subscriptions.on<CursorMoveToEvent>(CURSOR_MOVE_TO, this.handleCursorMoveTo);
        this.subscriptions.on<CursorRotateEvent>(CURSOR_ROTATE, this.handleCursorRotate);
        this.subscriptions.on<CursorRotateDeltaEvent>(CURSOR_ROTATE_DELTA, this.handleCursorRotateDelta);
        this.subscriptions.on<CursorZoomEvent>(CURSOR_ZOOM, this.handleCursorZoom);
        this.subscriptions.on<CursorZoomDeltaEvent>(CURSOR_ZOOM_DELTA, this.handleCursorZoomDelta);
        this.subscriptions.on<WorldReferenceChangedEvent>(WORLD_REFERENCE_CHANGED, this.handleWorldReferenceChanged);
    }

    update(deltaTime: number): void {
        // Integrate position
        this.position.x += this.velocity.x * deltaTime;
        this.position.y += this.velocity.y * deltaTime;
        this.position.z += this.velocity.z * deltaTime;

        // Integrate rotation rate
        if (this.rotateRate !== 0) {
            this.applyYawDelta(this.rotateRate * deltaTime);
        }

        // Integrate zoom rate
        if (this.zoomRate !== 0) {
            this.applyZoomDelta(this.zoomRate * deltaTime);
        }

        if (this.mesh) {
            this.mesh.position.copy(this.position);
            if (this.direction.lengthSq() > 0) {
                const angle = Math.atan2(this.direction.x, this.direction.y);
                this.mesh.rotation.z = angle;
            }
        }
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

    private applyYawDelta(angleDelta: number): void {
        const cos = Math.cos(angleDelta);
        const sin = Math.sin(angleDelta);

        this.cameraYaw += angleDelta;

        const fx = this.direction.x * cos - this.direction.y * sin;
        const fy = this.direction.x * sin + this.direction.y * cos;
        this.direction.set(fx, fy, 0);

        if (this.velocity.lengthSq() > 0) {
            const vx = this.velocity.x * cos - this.velocity.y * sin;
            const vy = this.velocity.x * sin + this.velocity.y * cos;
            this.velocity.set(vx, vy, 0);
        }
    }

    private handleCursorMove = (event: CursorMoveEvent): void => {
        const speed = CURSOR_MOVE_SPEED * (event.isSprinting ? CURSOR_SPRINT_MULTIPLIER : 1);
        this.velocity.copy(event.direction);
        this.velocity.multiplyScalar(speed);
    };

    private handleCursorMoveTo = (event: CursorMoveToEvent): void => {
        this.position.copy(event.pos);
    };

    // Stores rotate rate; applied continuously in update()
    private handleCursorRotate = (event: CursorRotateEvent): void => {
        this.rotateRate = event.direction;
    };

    // Applies an instant angle delta immediately
    private handleCursorRotateDelta = (event: CursorRotateDeltaEvent): void => {
        this.applyYawDelta(event.angleDelta);
    };

    // Stores zoom rate; applied continuously in update()
    private handleCursorZoom = (event: CursorZoomEvent): void => {
        this.zoomRate = event.direction;
    };

    // Applies an instant zoom delta immediately
    private handleCursorZoomDelta = (event: CursorZoomDeltaEvent): void => {
        this.applyZoomDelta(event.delta);
    };

    private applyZoomDelta(delta: number): void {
        const midDistance = (MAX_CAMERA_DISTANCE + MIN_CAMERA_DISTANCE) / 2;
        const zoomScale = (this.cameraDistance / midDistance) * ZOOM_DISTANCE_SCALE;

        this.cameraDistance += delta * zoomScale;
        this.cameraDistance = Math.max(MIN_CAMERA_DISTANCE, Math.min(MAX_CAMERA_DISTANCE, this.cameraDistance));
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
            0,
            headLength,
            0,
            -baseWidth,
            -5,
            0,
            baseWidth,
            -5,
            0
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
            const angle = Math.atan2(this.direction.x, this.direction.y);
            this.mesh.rotation.z = angle;
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
