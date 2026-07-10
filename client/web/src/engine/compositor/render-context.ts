import * as THREE from 'three';
import { WebGPURenderer } from 'three/webgpu';
import { PerformanceMetrics } from './performance-metrics';

export interface RenderOverlay {
    readonly scene: THREE.Scene;
    readonly camera: THREE.Camera;
}

export type RenderContextOptions = {
    setupScene?: boolean;
    showMetrics?: boolean;
};

export class RenderContext {
    readonly scene: THREE.Scene;
    readonly renderer: WebGPURenderer;
    readonly domElement: HTMLElement;
    private readonly resizeObserver: ResizeObserver;
    private readonly overlays: RenderOverlay[] = [];
    private readonly metrics: PerformanceMetrics | null;
    private _width: number;
    private _height: number;
    private readonly canvas: HTMLCanvasElement;

    constructor(container: HTMLElement, renderer: WebGPURenderer, options?: RenderContextOptions) {
        const setupScene = options?.setupScene ?? true;
        this.metrics = options?.showMetrics ? new PerformanceMetrics(renderer) : null;

        this.domElement = container;
        this.renderer = renderer;
        this.canvas = renderer.domElement;

        const width = container.clientWidth;
        const height = container.clientHeight;
        this._width = width;
        this._height = height;
        renderer.setSize(width, height);

        this.scene = new THREE.Scene();

        if (setupScene) {
            this.scene.background = new THREE.Color(0x1a1a2e);
            const ambient = new THREE.AmbientLight(0xffffff, 0.6);
            this.scene.add(ambient);
            const directional = new THREE.DirectionalLight(0xffffff, 0.8);
            directional.position.set(1000, -500, 3000);
            this.scene.add(directional);
        }

        this.resizeObserver = new ResizeObserver(() => {
            const w = container.clientWidth;
            const h = container.clientHeight;
            this._width = w;
            this._height = h;
            this.renderer.setSize(w, h);
        });
        this.resizeObserver.observe(container);
    }

    get width(): number {
        return this._width;
    }

    get height(): number {
        return this._height;
    }

    get aspect(): number {
        return this._width / this._height;
    }

    setMetricsVisible(visible: boolean): void {
        this.metrics?.setVisible(visible);
    }

    addOverlay(overlay: RenderOverlay): void {
        this.overlays.push(overlay);
    }

    removeOverlay(overlay: RenderOverlay): void {
        const i = this.overlays.indexOf(overlay);
        if (i !== -1) this.overlays.splice(i, 1);
    }

    render(camera: THREE.Camera, deltaTime: number): void {
        this.renderer.render(this.scene, camera);
        if (this.overlays.length) {
            this.renderer.autoClear = false;
            for (const o of this.overlays) this.renderer.render(o.scene, o.camera);
            this.renderer.autoClear = true;
        }
        this.metrics?.update(deltaTime);
    }

    dispose(): void {
        this.resizeObserver.disconnect();
        this.metrics?.dispose();
    }
}
