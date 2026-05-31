import type { Camera } from '../engine/camera/camera';
import type { GameSystem } from '../engine/game-system';
import type { RenderContext } from '../engine/render-context';

export class CameraViewportSystem implements GameSystem {
    readonly name = 'Camera Viewport';

    constructor(
        private readonly camera: Camera,
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
