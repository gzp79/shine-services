import { WebGPURenderer } from 'three/webgpu';
import { createCdtExperiment } from './experiments/cdt/index';
import { createHexMeshExperiment } from './experiments/hex-mesh/index';
import { createInputControlExperiment } from './experiments/input-control/index';
import { createInstancedColorMeshExperiment } from './experiments/instanced-color-mesh/index';
import { createTileChunkExperiment } from './experiments/tile-chunk/index';
import { createTrilinearExperiment } from './experiments/trilinear/index';
import { createWorldNeighborsExperiment } from './experiments/world-neighbors/index';
import { createGame } from './game/game';

export type Viewer = { dispose(): void };

type SceneId =
    | ''
    | 'hex-mesh'
    | 'cdt'
    | 'input-events'
    | 'trilinear'
    | 'world-neighbors'
    | 'tile-chunk'
    | 'instanced-color-mesh';

async function createSharedRenderer(): Promise<WebGPURenderer> {
    const renderer = new WebGPURenderer({ antialias: true, forceWebGL: false, powerPreference: 'high-performance' });
    await renderer.init();
    return renderer;
}

async function createContent(id: SceneId, container: HTMLElement, renderer: WebGPURenderer): Promise<Viewer> {
    switch (id) {
        case 'hex-mesh':
            return createHexMeshExperiment(container, renderer);
        case 'cdt':
            return createCdtExperiment(container, renderer);
        case 'input-events':
            return createInputControlExperiment(container, renderer);
        case 'trilinear':
            return createTrilinearExperiment(container, renderer);
        case 'world-neighbors':
            return createWorldNeighborsExperiment(container, renderer);
        case 'tile-chunk':
            return createTileChunkExperiment(container, renderer);
        case 'instanced-color-mesh':
            return createInstancedColorMeshExperiment(container, renderer);
        default:
            return createGame(container, renderer);
    }
}

export async function createScene(container: HTMLElement, id: SceneId): Promise<Viewer> {
    const renderer = await createSharedRenderer();
    container.appendChild(renderer.domElement);
    const scene = await createContent(id, container, renderer);

    return {
        dispose() {
            scene?.dispose();
            renderer.dispose();
            renderer.domElement.remove();
        }
    };
}

export async function createRoutedScene(container: HTMLElement) {
    const renderer = await createSharedRenderer();
    container.appendChild(renderer.domElement);

    let current: Viewer | null = null;

    async function navigate() {
        const hash = window.location.hash.replace('#', '') as SceneId;
        current?.dispose();
        current = null;
        current = await createContent(hash, container, renderer);
    }

    window.addEventListener('hashchange', () => void navigate());
    await navigate();

    return {
        dispose() {
            current?.dispose();
            renderer.dispose();
            renderer.domElement.remove();
        }
    };
}
