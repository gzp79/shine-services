import * as THREE from 'three';
import type { WorldCursor } from '../avatar/world-cursor';
import type { ICamera } from '../engine/camera/camera';
import type { IInputState } from '../engine/input/input-state';
import type { GameSystem } from '../engine/game-system';

export class CursorDriveSystem implements GameSystem {
    readonly name = 'Cursor Drive';

    constructor(
        private readonly cursor: WorldCursor,
        private readonly input: IInputState,
        private readonly camera: ICamera
    ) {}

    update(deltaTime: number): void {
        const { moveSpeed, rotateSpeed, zoomSpeed } = this.input;

        if (moveSpeed.x !== 0 || moveSpeed.y !== 0) {
            const yaw = this.cursor.cameraYaw;
            const forward = new THREE.Vector3(Math.sin(yaw), Math.cos(yaw), 0);
            const right = new THREE.Vector3(Math.cos(yaw), -Math.sin(yaw), 0);
            forward.multiplyScalar(moveSpeed.y * deltaTime);
            right.multiplyScalar(moveSpeed.x * deltaTime);
            this.cursor.moveBy(forward.add(right));
        }

        if (rotateSpeed !== 0) {
            this.cursor.rotateBy(rotateSpeed * deltaTime);
        }

        if (zoomSpeed !== 0) {
            this.cursor.zoomBy(zoomSpeed * deltaTime);
        }

        if (this.input.pendingMoveTo !== null) {
            const worldPos = this.camera.screenToWorldPlanePoint(this.input.pendingMoveTo.x, this.input.pendingMoveTo.y);
            if (worldPos) this.cursor.setPosition(worldPos);
        }

        if (this.input.pendingRotateBy !== 0) {
            this.cursor.rotateBy(this.input.pendingRotateBy);
        }

        if (this.input.pendingZoomBy !== 0) {
            this.cursor.zoomByDelta(this.input.pendingZoomBy);
        }

        if (this.input.pendingSchemaChange !== null) {
            this.cursor.dispatchSchemaChanged(this.input.pendingSchemaChange);
        }

        for (let i = 0; i < this.input.pendingInteractCount; i++) {
            const ev = this.input.pendingInteracts[i]!;
            const worldPos = this.camera.screenToWorldPlanePoint(ev.x, ev.y);
            if (worldPos) this.cursor.dispatchInteract(ev.kind, worldPos);
        }
    }

    dispose(): void {}
}
