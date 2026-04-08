import { Camera } from '../engine/camera/camera';
import { GameSystem } from '../engine/game-system';
import { RenderContext } from '../engine/render-context';
import { World } from '../world/world';

export class ChunkHoverSystem implements GameSystem {
    readonly name: string = 'Chunk Hover';

    constructor(
        private readonly world: World,
        private readonly renderContext: RenderContext,
        private readonly camera: Camera
    ) {}

    update(_deltaTime: number): void {
        const mousePosition = this.renderContext.mousePosition;
        if (mousePosition.x === -1 && mousePosition.y === -1) {
            this.world.clearHover();
            return;
        }

        const intersectionPoint = this.camera.ndcToWorldPlanePoint(mousePosition.x, mousePosition.y);
        if (intersectionPoint) {
            this.world.setHoverAt(intersectionPoint);
        } else {
            this.world.clearHover();
        }
    }

    dispose(): void {}
}
