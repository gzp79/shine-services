import * as THREE from 'three';
import type { WorldCursor } from './world-cursor';
import type { LocomotionState } from './input-manager';

const CURSOR_MOVE_SPEED = 10; // units/second
const CURSOR_SPRINT_MULTIPLIER = 2;
const CURSOR_ROTATE_SPEED = Math.PI; // radians/second
const CURSOR_ZOOM_SPEED = 10; // units/second

export class CursorLocomotionSystem {
    constructor(
        private readonly worldCursor: WorldCursor,
        private readonly getLocomotionState: () => Readonly<LocomotionState>
    ) {}

    update(deltaTime: number): void {
        const state = this.getLocomotionState();

        // Apply WASD movement (rotated by camera yaw)
        if (state.move.x !== 0 || state.move.y !== 0) {
            const { yaw } = this.worldCursor.getCameraTarget();

            // Camera-relative movement vectors
            const forward = new THREE.Vector3(Math.sin(yaw), Math.cos(yaw), 0);
            const right = new THREE.Vector3(Math.cos(yaw), -Math.sin(yaw), 0);

            // Combine forward/backward (y) and left/right (x)
            forward.multiplyScalar(-state.move.y);
            right.multiplyScalar(state.move.x);
            forward.add(right);
            forward.normalize();

            // Apply speed with sprint multiplier
            const speed = CURSOR_MOVE_SPEED * (state.sprint ? CURSOR_SPRINT_MULTIPLIER : 1);
            forward.multiplyScalar(speed * deltaTime);

            const newPosition = this.worldCursor.position.clone().add(forward);
            this.worldCursor.setPosition(newPosition);
        }

        // Apply Q/E rotation rate
        if (state.rotateRate !== 0) {
            const { yaw } = this.worldCursor.getCameraTarget();
            const newYaw = yaw + state.rotateRate * CURSOR_ROTATE_SPEED * deltaTime;
            this.worldCursor.setYaw(newYaw);
        }

        // Apply R/F zoom rate
        if (state.zoomRate !== 0) {
            const { distance } = this.worldCursor.getCameraTarget();
            const newDistance = distance + state.zoomRate * CURSOR_ZOOM_SPEED * deltaTime;
            this.worldCursor.setZoom(newDistance);
        }
    }
}
