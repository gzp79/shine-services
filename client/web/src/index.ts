import { WebGPURenderer } from 'three/webgpu';
import { createGame } from './engine/game';

export type Viewer = { dispose(): void };

async function createSharedRenderer(): Promise<WebGPURenderer> {
    const renderer = new WebGPURenderer({ antialias: true, forceWebGL: false });
    await renderer.init();
    return renderer;
}

export async function createRouter(container: HTMLElement) {
    const renderer = await createSharedRenderer();
    container.appendChild(renderer.domElement);

    const game = await createGame(container, renderer);

    return {
        dispose() {
            game.dispose();
            renderer.dispose();
            renderer.domElement.remove();
        }
    };
}
