import * as THREE from 'three';
import type { WorldCursor } from '../avatar/world-cursor';
import {
    CAMERA_BASE_LERP,
    CAMERA_LERP_DISTANCE_FACTOR,
    MAX_CAMERA_DISTANCE,
    MAX_CAMERA_PITCH,
    MIN_CAMERA_DISTANCE,
    MIN_CAMERA_PITCH
} from '../constants';
import type { Camera } from '../engine/camera/camera';
import { EventSubscriptions } from '../engine/events';
import type { GameSystem } from '../engine/game-system';
import { WORLD_REFERENCE_CHANGED, type WorldReferenceChangedEvent } from './world-reference-system';

export class CameraFollowCursorSystem implements GameSystem {
    private currentDistance: number;
    private currentYaw: number;
    private currentCursorPosition: THREE.Vector3;
    private currentLookAtPosition: THREE.Vector3;
    private readonly subscriptions: EventSubscriptions;

    constructor(
        private readonly camera: Camera,
        private readonly worldCursor: WorldCursor,
        events: EventTarget
    ) {
        this.subscriptions = new EventSubscriptions(events);

        // Initialize with WorldCursor's current parameters
        const target = worldCursor.getCameraTarget();
        this.currentDistance = target.distance;
        this.currentYaw = target.yaw;
        this.currentCursorPosition = target.cursorPosition.clone();
        this.currentLookAtPosition = target.lookAt.clone();

        // Listen to world reference changes to adjust blended positions
        this.subscriptions.on<WorldReferenceChangedEvent>(WORLD_REFERENCE_CHANGED, this.handleWorldReferenceChanged);
    }

    update(deltaTime: number): void {
        // Read all target parameters from WorldCursor
        const target = this.worldCursor.getCameraTarget();

        // Calculate adaptive lerp factor based on distance from target
        const distanceDiff = Math.abs(this.currentDistance - target.distance);
        const distanceFactor = Math.min(distanceDiff / 1000, 2);
        const lerpSpeed = CAMERA_BASE_LERP + distanceFactor * CAMERA_LERP_DISTANCE_FACTOR;

        // Apply deltaTime to make interpolation frame-rate independent
        const lerpFactor = 1 - Math.pow(1 - lerpSpeed, deltaTime * 60);

        // Blend all parameters toward target
        this.currentDistance += (target.distance - this.currentDistance) * lerpFactor;
        this.currentYaw += (target.yaw - this.currentYaw) * lerpFactor;
        this.currentCursorPosition.lerp(target.cursorPosition, lerpFactor);
        this.currentLookAtPosition.lerp(target.lookAt, lerpFactor);

        // Calculate pitch from distance (same formula as WorldCursor.getCameraTarget)
        const t = (this.currentDistance - MIN_CAMERA_DISTANCE) / (MAX_CAMERA_DISTANCE - MIN_CAMERA_DISTANCE);
        const pitch = MIN_CAMERA_PITCH + t * (MAX_CAMERA_PITCH - MIN_CAMERA_PITCH);

        // Calculate camera position from blended RTS parameters
        const height = this.currentDistance * Math.sin(pitch);
        const horizontalDist = this.currentDistance * Math.cos(pitch);

        const cameraPosition = new THREE.Vector3(
            this.currentCursorPosition.x - horizontalDist * Math.sin(this.currentYaw),
            this.currentCursorPosition.y - horizontalDist * Math.cos(this.currentYaw),
            this.currentCursorPosition.z + height
        );

        // Apply to camera
        this.camera.camera.position.copy(cameraPosition);
        this.camera.camera.lookAt(this.currentLookAtPosition);
    }

    dispose(): void {
        this.subscriptions.dispose();
    }

    private handleWorldReferenceChanged = (event: WorldReferenceChangedEvent): void => {
        // Adjust blended cursor and lookAt positions by the same delta as the world shift
        // This keeps the camera smoothly following without a discontinuity
        this.currentCursorPosition.x += event.deltaPosition.x;
        this.currentCursorPosition.y += event.deltaPosition.y;
        this.currentLookAtPosition.x += event.deltaPosition.x;
        this.currentLookAtPosition.y += event.deltaPosition.y;
    };
}
