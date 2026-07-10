import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { WebGPURenderer } from 'three/webgpu';
import type { Application } from '../engine/application';
import { DebugPanel } from '../engine/compositor/debug-panel';
import { RenderContext } from '../engine/compositor/render-context';
import { disposeObject3D } from '../engine/render/ownership';

export type ExperimentOption = {
    title: string;
    addOrbitCamera?: boolean;
};

export abstract class Experiment implements Application {
    readonly renderContext: RenderContext;
    readonly camera: THREE.PerspectiveCamera;
    readonly controls?: OrbitControls;
    readonly debugPanel: DebugPanel;

    get scene(): THREE.Scene {
        return this.renderContext.scene;
    }

    get renderer(): WebGPURenderer {
        return this.renderContext.renderer;
    }

    private animationId = 0;
    private lastTime = 0;
    private readonly _resizeObserver: ResizeObserver;

    constructor(container: HTMLElement, renderer: WebGPURenderer, options: ExperimentOption) {
        const addOrbitCamera = options.addOrbitCamera ?? true;

        this.renderContext = new RenderContext(container, renderer, { setupScene: false, showMetrics: true });
        this.debugPanel = new DebugPanel(container, options.title);

        const scene = this.renderContext.scene;
        scene.background = new THREE.Color(0x1a1a2e);
        const ambient = new THREE.AmbientLight(0xffffff, 0.6);
        scene.add(ambient);
        const directional = new THREE.DirectionalLight(0xffffff, 0.8);
        directional.position.set(10, -5, 20);
        scene.add(directional);

        const { width, height } = this.renderContext;
        this.camera = new THREE.PerspectiveCamera(50, width / height, 0.1, 100);
        this.camera.up.set(0, 0, 1);
        this.camera.position.set(0, -2.5, 4);
        this.camera.lookAt(0, 0, 0);

        if (addOrbitCamera) {
            this.controls = new OrbitControls(this.camera, renderer.domElement);
            this.controls.target.set(0, 0, 0);
            this.controls.enableDamping = true;
            this.controls.dampingFactor = 0.1;
            this.controls.update();
        }

        // Update camera aspect on resize — RenderContext already handles renderer resize
        const resizeObserver = new ResizeObserver(() => {
            this.camera.aspect = this.renderContext.aspect;
            this.camera.updateProjectionMatrix();
        });
        resizeObserver.observe(container);
        this._resizeObserver = resizeObserver;
    }

    protected onUpdate(_deltaTime: number): void {}

    start(): void {
        this.lastTime = performance.now();
        const tick = () => {
            const now = performance.now();
            const dt = (now - this.lastTime) / 1000;
            this.lastTime = now;
            this.onUpdate(dt);
            this.controls?.update();
            this.renderContext.render(this.camera, dt);
            this.animationId = requestAnimationFrame(tick);
        };
        tick();
    }

    dispose(): void {
        cancelAnimationFrame(this.animationId);
        this._resizeObserver.disconnect();
        this.debugPanel.dispose();
        this.controls?.dispose();
        this.renderContext.dispose();
        disposeObject3D(this.renderContext.scene);
        this.renderContext.scene.clear();
    }
}
