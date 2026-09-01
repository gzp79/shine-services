import { WebGPURenderer } from 'three/webgpu';
import type { Scene } from './scene';

function toError(value: unknown): Error {
    return value instanceof Error ? value : new Error(String(value));
}

// Owns the frame loop, the fatal-error boundary and teardown for a running Scene. A single owner
// so the contract is uniform across game and experiments: init failures reject createScene (see
// index.ts), runtime faults self-dispose then notify the host via onError.
export class SceneRuntime {
    private scene: Scene | null = null;
    private animationId = 0;
    private lastTime = 0;
    private disposed = false;

    constructor(
        private readonly renderer: WebGPURenderer,
        private readonly onError?: (error: Error) => void
    ) {}

    // Drives a scene's frame loop, replacing any currently-running one (routed navigation). The
    // first frame is scheduled async, so every loop fault is a runtime fault (onError), never a
    // createScene rejection. A synchronous throw from init propagates to the caller instead.
    run(scene: Scene): void {
        if (this.disposed) return;
        this.scene?.dispose();
        this.scene = scene;
        scene.init?.(this);
        this.lastTime = performance.now();
        this.animationId = requestAnimationFrame(this.loop);
    }

    // Launches detached async work; a rejection becomes a fatal error. The single blessed way to
    // fire background work from a scene — see no-floating-promises / no-misused-promises.
    spawn(promise: Promise<unknown>): void {
        promise.catch((error: unknown) => this.reportFatal(toError(error)));
    }

    // Fatal fault on a live scene: release everything, then hand the error to the host. Guarded so
    // a late rejection arriving after teardown is dropped rather than fired at a dead scene.
    reportFatal(error: Error): void {
        if (this.disposed) return;
        this.dispose();
        this.onError?.(error);
    }

    setInputEnabled(enabled: boolean): void {
        this.scene?.setInputEnabled?.(enabled);
    }

    dispose(): void {
        if (this.disposed) return;
        this.disposed = true;
        cancelAnimationFrame(this.animationId);
        this.scene?.dispose();
        this.scene = null;
        this.renderer.dispose();
        this.renderer.domElement.remove();
    }

    private readonly loop = (): void => {
        const now = performance.now();
        const dt = (now - this.lastTime) / 1000;
        this.lastTime = now;
        try {
            this.scene?.frame(dt);
        } catch (error) {
            this.reportFatal(toError(error));
            return;
        }
        this.animationId = requestAnimationFrame(this.loop);
    };
}
