import { WebGPURenderer } from 'three/webgpu';
import { createGame } from './engine/game';
import { createSharedRenderer } from './experiments/experiment';
import { createCdtExperiment } from './experiments/cdt/index';
import { createHexMeshExperiment } from './experiments/hex-mesh/index';
import { createInputControlExperiment } from './experiments/input-control/index';
import { createTrilinearExperiment } from './experiments/trilinear/index';
import { createWorldNeighborsExperiment } from './experiments/world-neighbors/index';

export type Scene = 'game' | 'hex-mesh' | 'cdt' | 'input-events' | 'trilinear' | 'world-neighbors';
export type Viewer = { dispose(): void };

export async function createScene(container: HTMLElement, scene: Scene, renderer: WebGPURenderer): Promise<Viewer> {
    switch (scene) {
        case 'hex-mesh':
            return await createHexMeshExperiment(container, renderer);
        case 'cdt':
            return await createCdtExperiment(container, renderer);
        case 'input-events':
            return await createInputControlExperiment(container, renderer);
        case 'trilinear':
            return await createTrilinearExperiment(container, renderer);
        case 'world-neighbors':
            return await createWorldNeighborsExperiment(container, renderer);
        case 'game':
        default:
            return createGame(container, renderer);
    }
}

const hashToScene: Record<string, Scene> = {
    '#hex-mesh': 'hex-mesh',
    '#cdt': 'cdt',
    '#input-events': 'input-events',
    '#trilinear': 'trilinear',
    '#world-neighbors': 'world-neighbors'
};

export async function createRouter(container: HTMLElement) {
    const renderer = await createSharedRenderer();
    container.appendChild(renderer.domElement);

    let current: Viewer | null = null;

    async function route() {
        if (current) {
            current.dispose();
            current = null;
        }

        const hash = window.location.hash;
        const scene = hashToScene[hash] ?? 'game';

        try {
            current = await createScene(container, scene, renderer);
        } catch (e) {
            console.error('Scene failed to load:', e);
        }
    }

    window.addEventListener('hashchange', () => void route());
    void route();

    return {
        dispose() {
            if (current) {
                current.dispose();
                current = null;
            }
            renderer.dispose();
            renderer.domElement.remove();
        }
    };
}
