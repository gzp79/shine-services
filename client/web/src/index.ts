import init from '#wasm';
import wasmUrl from '#wasm-bin';
import { WebGPURenderer } from 'three/webgpu';
import type { Application } from './engine/application';
import { AssetViewer } from './experiments/asset-viewer/index';
import { Cdt } from './experiments/cdt/index';
import { HexMesh } from './experiments/hex-mesh/index';
import { InputControl } from './experiments/input-control/index';
import { InstancedColorMeshExp } from './experiments/instanced-color-mesh/index';
import { TileChunk } from './experiments/tile-chunk/index';
import { Trilinear } from './experiments/trilinear/index';
import { WorldNeighbors } from './experiments/world-neighbors/index';
import { Game } from './game/game';

export type { Application } from './engine/application';

type SceneId =
    | ''
    | 'hex-mesh'
    | 'cdt'
    | 'input-events'
    | 'trilinear'
    | 'world-neighbors'
    | 'tile-chunk'
    | 'instanced-color-mesh'
    | 'asset-viewer';

async function createSharedRenderer(): Promise<WebGPURenderer> {
    const renderer = new WebGPURenderer({ antialias: true, forceWebGL: false, powerPreference: 'high-performance' });
    await renderer.init();
    await init({ module_or_path: wasmUrl });
    return renderer;
}

function createContent(id: SceneId, container: HTMLElement, renderer: WebGPURenderer): Application {
    switch (id) {
        case 'hex-mesh':
            return new HexMesh(container, renderer);
        case 'cdt':
            return new Cdt(container, renderer);
        case 'input-events':
            return new InputControl(container, renderer);
        case 'trilinear':
            return new Trilinear(container, renderer);
        case 'world-neighbors':
            return new WorldNeighbors(container, renderer);
        case 'tile-chunk':
            return new TileChunk(container, renderer);
        case 'instanced-color-mesh':
            return new InstancedColorMeshExp(container, renderer);
        case 'asset-viewer':
            return new AssetViewer(container, renderer);
        default:
            return new Game(container, renderer);
    }
}

export async function createScene(container: HTMLElement, id: SceneId): Promise<{ dispose(): void }> {
    const renderer = await createSharedRenderer();
    container.appendChild(renderer.domElement);
    const content = createContent(id, container, renderer);
    content.start();

    return {
        dispose() {
            content?.dispose();
            renderer.dispose();
            renderer.domElement.remove();
        }
    };
}

export async function createRoutedScene(container: HTMLElement): Promise<{ dispose(): void }> {
    const renderer = await createSharedRenderer();
    container.appendChild(renderer.domElement);

    let current: Application | null = null;

    async function navigate() {
        const hash = window.location.hash.replace('#', '') as SceneId;
        current?.dispose();
        current = null;
        current = createContent(hash, container, renderer);
        current.start();
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
