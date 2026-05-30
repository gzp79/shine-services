import * as THREE from 'three';
import { WebGPURenderer } from 'three/webgpu';
export class RenderContext {
    readonly scene: THREE.Scene;
    readonly renderer: WebGPURenderer;
    readonly domElement: HTMLElement;
    private readonly resizeObserver: ResizeObserver;
    private _width: number;
    private _height: number;
    private readonly canvas: HTMLCanvasElement;
    get width(): number {
        return this._width;
    }
    get height(): number {
        return this._height;
    }
    get aspect(): number {
        return this._width / this._height;
    }

    constructor(container: HTMLElement, renderer: WebGPURenderer) {
        this.domElement = container;
        this.renderer = renderer;
        this.canvas = renderer.domElement;

        const width = container.clientWidth;
        const height = container.clientHeight;
        this._width = width;
        this._height = height;
        renderer.setSize(width, height);

        this.scene = new THREE.Scene();
        this.scene.background = new THREE.Color(0x1a1a2e);

        const ambient = new THREE.AmbientLight(0xffffff, 0.6);
        this.scene.add(ambient);
        const directional = new THREE.DirectionalLight(0xffffff, 0.8);
        directional.position.set(1000, -500, 3000);
        this.scene.add(directional);

        this.resizeObserver = new ResizeObserver(() => {
            const w = container.clientWidth;
            const h = container.clientHeight;
            this._width = w;
            this._height = h;
            this.renderer.setSize(w, h);
        });
        this.resizeObserver.observe(container);

    }

    async render(camera: THREE.PerspectiveCamera): Promise<void> {
        await this.renderer.renderAsync(this.scene, camera);
    }

    dispose(): void {
        this.resizeObserver.disconnect();
    }
}
