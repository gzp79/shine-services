import type { RtsCamera } from '../avatar/rts-camera';
import type { RenderContext } from '../engine/render-context';
import type { GameSystem } from '../game-system';

export class CameraViewportSystem implements GameSystem {
    readonly name = 'Camera Viewport';

    constructor(
        private readonly camera: RtsCamera,
        private readonly renderContext: RenderContext
    ) {}

    update(_deltaTime: number): void {
        const aspect = this.renderContext.aspect;
        if (aspect !== this.camera.aspect) {
            this.camera.width = this.renderContext.width;
            this.camera.height = this.renderContext.height;
            this.camera.camera.aspect = aspect;
            this.camera.camera.updateProjectionMatrix();
        }
    }

    dispose(): void {}
}
