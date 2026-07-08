import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { WebGPURenderer } from 'three/webgpu';
import { disposeObject3D } from '../engine/render/ownership';

export type ExperimentOption = {
    addOrbitCamera?: boolean;
};

export async function createSharedRenderer(): Promise<WebGPURenderer> {
    const renderer = new WebGPURenderer({ antialias: true, powerPreference: 'high-performance' });
    await renderer.init();
    renderer.setPixelRatio(window.devicePixelRatio);
    return renderer;
}

export abstract class Experiment {
    readonly scene: THREE.Scene;
    readonly camera: THREE.PerspectiveCamera;
    readonly renderer: WebGPURenderer;
    readonly controls?: OrbitControls;

    private readonly resizeObserver: ResizeObserver;
    private animationId = 0;
    private lastTime = 0;

    constructor(container: HTMLElement, renderer: WebGPURenderer, options?: ExperimentOption) {
        const addOrbitCamera = options?.addOrbitCamera ?? true;

        this.renderer = renderer;

        this.scene = new THREE.Scene();
        this.scene.background = new THREE.Color(0x1a1a2e);

        const width = container.clientWidth;
        const height = container.clientHeight;

        this.camera = new THREE.PerspectiveCamera(50, width / height, 0.1, 100);
        this.camera.up.set(0, 0, 1);
        this.camera.position.set(0, -2.5, 4);
        this.camera.lookAt(0, 0, 0);

        renderer.setSize(width, height);

        if (addOrbitCamera) {
            this.controls = new OrbitControls(this.camera, renderer.domElement);
            this.controls.target.set(0, 0, 0);
            this.controls.enableDamping = true;
            this.controls.dampingFactor = 0.1;
            this.controls.update();
        }

        const ambient = new THREE.AmbientLight(0xffffff, 0.6);
        this.scene.add(ambient);
        const directional = new THREE.DirectionalLight(0xffffff, 0.8);
        directional.position.set(10, -5, 20);
        this.scene.add(directional);

        this.resizeObserver = new ResizeObserver(() => {
            const w = container.clientWidth;
            const h = container.clientHeight;
            this.camera.aspect = w / h;
            this.camera.updateProjectionMatrix();
            this.renderer.setSize(w, h);
        });
        this.resizeObserver.observe(container);
    }

    protected onUpdate(_deltaTime: number): void {}
    protected onPostRender(_renderer: WebGPURenderer): Promise<void> | void {}

    start(): void {
        this.lastTime = performance.now();
        const loop = async () => {
            const now = performance.now();
            const deltaTime = (now - this.lastTime) / 1000;
            this.lastTime = now;
            this.onUpdate(deltaTime);
            this.controls?.update();
            await this.renderer.renderAsync(this.scene, this.camera);
            await this.onPostRender(this.renderer);
            this.animationId = requestAnimationFrame(loop);
        };
        void loop();
    }

    dispose(): void {
        cancelAnimationFrame(this.animationId);
        this.controls?.dispose();
        this.resizeObserver.disconnect();
        disposeObject3D(this.scene);
        this.scene.clear();
    }
}
