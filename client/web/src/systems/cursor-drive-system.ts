import type { WorldCursor } from '../avatar/world-cursor';
import type { ICamera } from '../engine/camera/camera';
import type { GameSystem } from '../engine/game-system';
import type { IInputState } from '../engine/input/input-state';

export class CursorDriveSystem implements GameSystem {
    readonly name = 'Cursor Drive';

    constructor(
        private readonly cursor: WorldCursor,
        private readonly input: IInputState,
        private readonly camera: ICamera
    ) {}

    update(deltaTime: number): void {
        const { moveSpeed, rotateSpeed, zoomSpeed } = this.input;

        if (rotateSpeed !== 0) {
            this.cursor.rotateBy(rotateSpeed * deltaTime);
        }

        if (moveSpeed.x !== 0 || moveSpeed.y !== 0) {
            this.cursor.moveBy(moveSpeed.y * deltaTime, moveSpeed.x * deltaTime, 0);
        }

        if (zoomSpeed !== 0) {
            this.cursor.zoomBy(zoomSpeed * deltaTime);
        }

        if (this.input.pendingMoveTo !== null) {
            const worldPos = this.camera.screenToWorldPlanePoint(
                this.input.pendingMoveTo.x,
                this.input.pendingMoveTo.y
            );
            if (worldPos) this.cursor.setPosition(worldPos);
        }

        if (this.input.pendingRotateBy !== 0) {
            this.cursor.rotateBy(this.input.pendingRotateBy);
        }

        if (this.input.pendingZoomBy !== 0) {
            this.cursor.zoomBy(this.input.pendingZoomBy);
        }
    }

    dispose(): void {}
}
