//import { createGame } from './engine/game';
//import { createCdtExperiment } from './experiments/cdt/index';
import { createHexMeshExperiment } from './experiments/hex-mesh/index';
import { createInputControlExperiment } from './experiments/input-control/index';
import { createTrilinearExperiment } from './experiments/trilinear/index';

//import { createWorldNeighborsExperiment } from './experiments/world-neighbors/index';

export type Scene = 'game' | 'hex-mesh' | 'cdt' | 'input-events' | 'trilinear' | 'world-neighbors';
export type Viewer = { dispose(): void };

export async function createScene(container: HTMLElement, scene: Scene): Promise<Viewer> {
    switch (scene) {
        case 'hex-mesh': {
            return await createHexMeshExperiment(container);
        }
        /*case 'cdt': {
            return await createCdtExperiment(container);
            }*/
        case 'input-events': {
            return await createInputControlExperiment(container);
        }
        case 'trilinear': {
            return await createTrilinearExperiment(container);
        }
        /*case 'world-neighbors': {
            return await createWorldNeighborsExperiment(container);
            }*/
        /*case 'game':
        default: {
            return createGame(container);
            }*/
    }
}

const hashToScene: Record<string, Scene> = {
    '#hex-mesh': 'hex-mesh',
    '#cdt': 'cdt',
    '#input-events': 'input-events',
    '#trilinear': 'trilinear',
    '#world-neighbors': 'world-neighbors'
};

export function createRouter(container: HTMLElement) {
    let current: Viewer | null = null;

    async function route() {
        if (current) {
            current.dispose();
            current = null;
        }

        const hash = window.location.hash;
        const scene = hashToScene[hash] ?? 'game';

        try {
            current = await createScene(container, scene);
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
        }
    };
}
