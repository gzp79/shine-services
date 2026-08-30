import init from '#wasm';
import wasmUrl from '#wasm-bin';
import { WebGPURenderer } from 'three/webgpu';
import type { Application } from './engine/application';
import { createContent, scenes } from './scene-registry';

export type { Application } from './engine/application';

/** Scenes available in this bundle, for consumers to build navigation. */
export function listScenes(): { id: string; title: string }[] {
    return scenes.map(({ id, title }) => ({ id, title }));
}

async function createSharedRenderer(): Promise<WebGPURenderer> {
    const renderer = new WebGPURenderer({ antialias: true, forceWebGL: false, powerPreference: 'high-performance' });
    await renderer.init();
    await init({ module_or_path: wasmUrl });
    return renderer;
}

export async function createScene(
    container: HTMLElement,
    id: string
): Promise<{ dispose(): void; setInputEnabled?: (enabled: boolean) => void }> {
    const renderer = await createSharedRenderer();
    container.appendChild(renderer.domElement);
    const content = createContent(id, container, renderer);
    content.start();

    return {
        dispose() {
            content?.dispose();
            renderer.dispose();
            renderer.domElement.remove();
        },
        setInputEnabled: (enabled) => content.setInputEnabled?.(enabled)
    };
}

export async function createRoutedScene(container: HTMLElement): Promise<{ dispose(): void }> {
    const renderer = await createSharedRenderer();
    container.appendChild(renderer.domElement);

    let current: Application | null = null;

    async function navigate() {
        const hash = window.location.hash.replace('#', '');
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
