import init from '#wasm';
import wasmUrl from '#wasm-bin';
import { WebGPURenderer } from 'three/webgpu';
import type { AssetCatalogBuilder } from './engine/assets/catalog';
import { RendererUnavailableError, hasRenderBackend } from './engine/compositor/render-backend';
import type { StreamHub } from './engine/net/stream-hub';
import { SceneRuntime } from './engine/scene-runtime';
import { createContent, scenes } from './scene-registry';

export type { StreamHub, StreamListener, StreamSubscription } from './engine/net/stream-hub';

export interface SceneHandle {
    dispose(): void;
    setInputEnabled(enabled: boolean): void;
}

// Reports a fatal fault after the bundle releases the scene. Synchronous startup faults also reject
// createScene after invoking this callback; live async and frame faults only use the callback.
export type SceneErrorHandler = (error: Error) => void;

export interface SceneConfig {
    catalogBuilder: AssetCatalogBuilder;
    onError?: SceneErrorHandler;
    streamHub?: StreamHub;
}

/** Scenes available in this bundle, for consumers to build navigation. */
export function listScenes(): { id: string; title: string }[] {
    return scenes.map(({ id, title }) => ({ id, title }));
}

async function createSharedRenderer(): Promise<WebGPURenderer> {
    if (!(await hasRenderBackend())) throw new RendererUnavailableError();
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

export async function createScene(container: HTMLElement, id: string, config: SceneConfig): Promise<SceneHandle> {
    const renderer = await createSharedRenderer();
    const runtime = new SceneRuntime(renderer, config.onError);
    try {
        container.appendChild(renderer.domElement);
        runtime.run((runtime) =>
            createContent(id, {
                container,
                renderer,
                runtime,
                catalogBuilder: config.catalogBuilder,
                streamHub: config.streamHub
            })
        );
    } catch (error) {
        runtime.dispose();
        throw error;
    }
    return handle(runtime);
}

export async function createRoutedScene(container: HTMLElement, config: SceneConfig): Promise<SceneHandle> {
    const renderer = await createSharedRenderer();
    const runtime = new SceneRuntime(renderer, config.onError);

    function navigate(): void {
        const hash = window.location.hash.replace('#', '');
        runtime.run((runtime) =>
            createContent(hash, {
                container,
                renderer,
                runtime,
                catalogBuilder: config.catalogBuilder,
                streamHub: config.streamHub
            })
        );
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
