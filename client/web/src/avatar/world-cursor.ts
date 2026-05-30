import * as THREE from 'three';
import {
    CURSOR_MOVE_SPEED,
    CURSOR_SPRINT_MULTIPLIER,
    CURSOR_ROTATE_SPEED,
    CURSOR_ZOOM_SPEED,
    MAX_CAMERA_DISTANCE,
    MAX_CAMERA_PITCH,
    MIN_CAMERA_DISTANCE,
    MIN_CAMERA_PITCH,
    ZOOM_DISTANCE_SCALE
} from '../constants';
import { EventSubscriptions } from '../engine/events';
import type { RenderContext } from '../engine/render-context';
import { WORLD_REFERENCE_CHANGED, type WorldReferenceChangedEvent } from '../systems/world-reference-system';

export class WorldCursor {
    readonly position = new THREE.Vector3(0, 0, 0);
    private readonly direction = new THREE.Vector3(0, 1, 0);
    private cameraDistance = 600;
    private cameraYaw = 0;
    private mesh: THREE.Mesh | null = null;
    private readonly subscriptions: EventSubscriptions;

    // Rate state written directly by CursorInputSystem each input callback
    readonly moveRate = new THREE.Vector2(0, 0);
    moveRateSprint = false;
    rotateRate = 0;
    zoomRate = 0;

    constructor(
        private readonly renderContext: RenderContext,
        events: EventTarget
    ) {
        this.subscriptions = new EventSubscriptions(events);
        this.subscriptions.on<WorldReferenceChangedEvent>(WORLD_REFERENCE_CHANGED, this.handleWorldReferenceChanged);
    }

    update(deltaTime: number): void {
        if (this.moveRate.x !== 0 || this.moveRate.y !== 0) {
            const forward = new THREE.Vector3(Math.sin(this.cameraYaw), Math.cos(this.cameraYaw), 0);
            const right = new THREE.Vector3(Math.cos(this.cameraYaw), -Math.sin(this.cameraYaw), 0);

            forward.multiplyScalar(-this.moveRate.y);
            right.multiplyScalar(this.moveRate.x);
            forward.add(right).normalize();

            const speed = CURSOR_MOVE_SPEED * (this.moveRateSprint ? CURSOR_SPRINT_MULTIPLIER : 1);
            forward.multiplyScalar(speed * deltaTime);

            this.setPosition(this.position.clone().add(forward));
        }

        if (this.rotateRate !== 0) {
            this.setYaw(this.cameraYaw + this.rotateRate * CURSOR_ROTATE_SPEED * deltaTime);
        }

        if (this.zoomRate !== 0) {
            this.setZoom(this.cameraDistance + this.zoomRate * CURSOR_ZOOM_SPEED * deltaTime);
        }
    }

    setPosition(pos: { x: number; y: number; z: number }): void {
        this.position.set(pos.x, pos.y, pos.z);
        if (this.mesh) {
            this.mesh.position.copy(this.position);
        }
    }

    setYaw(yaw: number): void {
        const TWO_PI = 2 * Math.PI;
        this.cameraYaw = ((yaw % TWO_PI) + TWO_PI) % TWO_PI;
        this.direction.set(Math.sin(this.cameraYaw), Math.cos(this.cameraYaw), 0);
        if (this.mesh) {
            this.mesh.rotation.z = Math.atan2(this.direction.x, this.direction.y);
        }
    }

    setZoom(distance: number): void {
        this.cameraDistance = Math.max(MIN_CAMERA_DISTANCE, Math.min(MAX_CAMERA_DISTANCE, distance));
    }

    rotateBy(angleDelta: number): void {
        this.setYaw(this.cameraYaw + angleDelta);
    }

    zoomBy(delta: number): void {
        const midDistance = (MAX_CAMERA_DISTANCE + MIN_CAMERA_DISTANCE) / 2;
        const zoomScale = (this.cameraDistance / midDistance) * ZOOM_DISTANCE_SCALE;
        this.setZoom(this.cameraDistance + delta * zoomScale);
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
