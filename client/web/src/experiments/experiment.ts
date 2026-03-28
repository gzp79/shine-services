import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';

export interface ExperimentContext {
    scene: THREE.Scene;
    camera: THREE.PerspectiveCamera;
    renderer: THREE.WebGLRenderer;
    controls: OrbitControls;
    resizeObserver: ResizeObserver;
}

export function createExperiment(container: HTMLElement): ExperimentContext {
    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x1a1a2e);

    const width = container.clientWidth;
    const height = container.clientHeight;

    const camera = new THREE.PerspectiveCamera(50, width / height, 0.1, 100);
    camera.up.set(0, 0, 1);
    camera.position.set(0, -2.5, 4);
    camera.lookAt(0, 0, 0);

    const renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setSize(width, height);
    renderer.setPixelRatio(window.devicePixelRatio);
    container.appendChild(renderer.domElement);

    const controls = new OrbitControls(camera, renderer.domElement);
    controls.target.set(0, 0, 0);
    controls.enableDamping = true;
    controls.dampingFactor = 0.1;
    controls.update();

    // Lighting
    const ambient = new THREE.AmbientLight(0xffffff, 0.6);
    scene.add(ambient);

    const directional = new THREE.DirectionalLight(0xffffff, 0.8);
    directional.position.set(10, -5, 20);
    scene.add(directional);

    // Resize handling via ResizeObserver on container
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

export function animate(ctx: ExperimentContext): number {
    let id = 0;
    function loop() {
        id = requestAnimationFrame(loop);
        ctx.controls.update();
        ctx.renderer.render(ctx.scene, ctx.camera);
    }
    loop();
    return id;
}
