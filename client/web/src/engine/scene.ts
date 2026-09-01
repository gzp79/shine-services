import type { WebGPURenderer } from 'three/webgpu';
import type { AssetCatalogBuilder } from './assets/catalog';
import type { SceneRuntime } from './scene-runtime';

export interface SceneContext {
    readonly container: HTMLElement;
    readonly renderer: WebGPURenderer;
    readonly runtime: SceneRuntime;
    readonly catalogBuilder: AssetCatalogBuilder;
}

// A unit of content the SceneRuntime drives: the shipped game or a dev experiment. The runtime
// owns the frame loop, fault boundary and lifecycle; a scene only advances one frame and, if it
// runs async work, routes it through the runtime (init).
export interface Scene {
    // One-time setup once the runtime is ready. Kick off preloads via runtime.spawn here so a
    // rejection becomes a fatal error; a synchronous throw fails scene creation (rejects createScene).
    init?(): void;
    frame(dt: number): void;
    setInputEnabled?(enabled: boolean): void;
    dispose(): void;
}
