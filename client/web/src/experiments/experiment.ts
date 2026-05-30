import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { WebGPURenderer } from 'three/webgpu';

export type ExperimentOption = {
    addOrbitCamera?: boolean;
};

export interface ExperimentContext {
    scene: THREE.Scene;
    camera: THREE.PerspectiveCamera;
    renderer: WebGPURenderer;
    controls?: OrbitControls;
    resizeObserver: ResizeObserver;
}

export async function createSharedRenderer(): Promise<WebGPURenderer> {
    const renderer = new WebGPURenderer({ antialias: true });
    await renderer.init();
    renderer.setPixelRatio(window.devicePixelRatio);
    return renderer;
}

export function createExperiment(
    container: HTMLElement,
    renderer: WebGPURenderer,
    options?: ExperimentOption
): ExperimentContext {
    const addOrbitCamera = options?.addOrbitCamera ?? true;

    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x1a1a2e);

    const width = container.clientWidth;
    const height = container.clientHeight;

    const camera = new THREE.PerspectiveCamera(50, width / height, 0.1, 100);
    camera.up.set(0, 0, 1);
    camera.position.set(0, -2.5, 4);
    camera.lookAt(0, 0, 0);

    renderer.setSize(width, height);

    let controls: OrbitControls | undefined = undefined;
    if (addOrbitCamera) {
        controls = new OrbitControls(camera, renderer.domElement);
        controls.target.set(0, 0, 0);
        controls.enableDamping = true;
        controls.dampingFactor = 0.1;
        controls.update();
    }

    const ambient = new THREE.AmbientLight(0xffffff, 0.6);
    scene.add(ambient);
    const directional = new THREE.DirectionalLight(0xffffff, 0.8);
    directional.position.set(10, -5, 20);
    scene.add(directional);

    const resizeObserver = new ResizeObserver(() => {
        const w = container.clientWidth;
        const h = container.clientHeight;
        camera.aspect = w / h;
        camera.updateProjectionMatrix();
        renderer.setSize(w, h);
    });
    resizeObserver.observe(container);

    return { scene, camera, renderer, controls, resizeObserver };
}

export function animate(ctx: ExperimentContext, onFrame?: () => void): () => void {
    let id = 0;
    function loop() {
        id = requestAnimationFrame(loop);
        onFrame?.();
        ctx.controls?.update();
        void ctx.renderer.renderAsync(ctx.scene, ctx.camera);
    }
    loop();
    return () => cancelAnimationFrame(id);
}

export function disposeExperiment(ctx: ExperimentContext) {
    ctx.controls?.dispose();
    ctx.resizeObserver.disconnect();
    disposeObject3D(ctx.scene);
    ctx.scene.clear();
}

type DisposableMesh = THREE.Object3D & {
    geometry: THREE.BufferGeometry;
    material: THREE.Material | THREE.Material[];
};

export function disposeMesh(mesh: DisposableMesh) {
    mesh.geometry?.dispose();
    if (Array.isArray(mesh.material)) {
        mesh.material.forEach((mat) => mat.dispose());
    } else {
        mesh.material?.dispose();
    }
}

export function disposeObject3D(obj: THREE.Object3D) {
    obj.traverse((child) => {
        if (child instanceof THREE.Mesh) {
            disposeMesh(child);
        }
    });
}
