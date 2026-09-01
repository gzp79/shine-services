import init from '#wasm';
import wasmUrl from '#wasm-bin';
import { WebGPURenderer } from 'three/webgpu';
import type { AssetCatalogBuilder } from './engine/assets/catalog';
import { SceneRuntime } from './engine/scene-runtime';
import { createContent, scenes } from './scene-registry';

export interface SceneHandle {
    dispose(): void;
    setInputEnabled(enabled: boolean): void;
}

// Reports a fatal fault after the scene is live. The bundle has already released the scene by the
// time this fires; the host swaps in its own error UI. Init failures reject createScene instead.
export type SceneErrorHandler = (error: Error) => void;

/** Scenes available in this bundle, for consumers to build navigation. */
export function listScenes(): { id: string; title: string }[] {
    return scenes.map(({ id, title }) => ({ id, title }));
}

async function createSharedRenderer(): Promise<WebGPURenderer> {
    const renderer = new WebGPURenderer({ antialias: true, forceWebGL: false, powerPreference: 'high-performance' });
    try {
        await renderer.init();
        await init({ module_or_path: wasmUrl });
    } catch (error) {
        renderer.dispose();
        throw error;
    }
    return renderer;
}

export async function createScene(
    container: HTMLElement,
    id: string,
    catalogBuilder: AssetCatalogBuilder,
    onError?: SceneErrorHandler
): Promise<SceneHandle> {
    const renderer = await createSharedRenderer();
    const runtime = new SceneRuntime(renderer, onError);
    try {
        container.appendChild(renderer.domElement);
        runtime.run(createContent(id, container, renderer, catalogBuilder));
    } catch (error) {
        runtime.dispose();
        throw error;
    }
    return handle(runtime);
}

export async function createRoutedScene(
    container: HTMLElement,
    catalogBuilder: AssetCatalogBuilder,
    onError?: SceneErrorHandler
): Promise<SceneHandle> {
    const renderer = await createSharedRenderer();
    const runtime = new SceneRuntime(renderer, onError);

    function navigate(): void {
        const hash = window.location.hash.replace('#', '');
        runtime.run(createContent(hash, container, renderer, catalogBuilder));
    }

    try {
        container.appendChild(renderer.domElement);
        navigate();
    } catch (error) {
        runtime.dispose();
        throw error;
    }

    // A navigation after startup can no longer reject createRoutedScene, so a failed rebuild is a
    // runtime fault routed through the same channel as a live fault.
    window.addEventListener('hashchange', () => {
        try {
            navigate();
        } catch (error) {
            runtime.reportFatal(error instanceof Error ? error : new Error(String(error)));
        }
    });

    return handle(runtime);
}

function handle(runtime: SceneRuntime): SceneHandle {
    return {
        dispose: () => runtime.dispose(),
        setInputEnabled: (enabled) => runtime.setInputEnabled(enabled)
    };
}
