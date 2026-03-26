import init from '#wasm';
import wasmUrl from '#wasm-bin';
import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { ChunkId } from './world/types';
import { World } from './world/world';

class Game {
    private readonly scene: THREE.Scene;
    private readonly camera: THREE.PerspectiveCamera;
    private readonly renderer: THREE.WebGLRenderer;
    private readonly controls: OrbitControls;
    private readonly resizeObserver: ResizeObserver;
    private readonly world: World;
    private animationId = 0;

    constructor(private readonly container: HTMLElement) {
        this.scene = new THREE.Scene();
        this.scene.background = new THREE.Color(0x1a1a2e);

        const width = container.clientWidth;
        const height = container.clientHeight;

        this.camera = new THREE.PerspectiveCamera(50, width / height, 1, 50000);
        this.camera.up.set(0, 0, 1);
        this.camera.position.set(0, -1200, 2000);
        this.camera.lookAt(0, 0, 0);

        this.renderer = new THREE.WebGLRenderer({ antialias: true });
        this.renderer.setSize(width, height);
        this.renderer.setPixelRatio(window.devicePixelRatio);
        container.appendChild(this.renderer.domElement);

        this.controls = new OrbitControls(this.camera, this.renderer.domElement);
        this.controls.target.set(0, 0, 0);
        this.controls.enableDamping = true;
        this.controls.dampingFactor = 0.1;
        this.controls.update();

        // Lighting
        const ambient = new THREE.AmbientLight(0xffffff, 0.6);
        this.scene.add(ambient);

        const directional = new THREE.DirectionalLight(0xffffff, 0.8);
        directional.position.set(1000, -500, 3000);
        this.scene.add(directional);

        // Resize handling
        this.resizeObserver = new ResizeObserver(() => {
            const w = container.clientWidth;
            const h = container.clientHeight;
            this.camera.aspect = w / h;
            this.camera.updateProjectionMatrix();
            this.renderer.setSize(w, h);
        });
        this.resizeObserver.observe(container);

        // World
        this.world = new World();
        this.scene.add(this.world.group);
    }

    init(): void {
        this.world.loadChunk(ChunkId.ORIGIN);
        for (const neighbor of ChunkId.ORIGIN.neighbors()) {
            this.world.loadChunk(neighbor);
        }

        // Debug: circle with radius 1000 at origin (XY plane)
        const circleGeom = new THREE.RingGeometry(998, 1000, 64);
        const circleMat = new THREE.MeshBasicMaterial({ color: 0xff0000, side: THREE.DoubleSide });
        const circle = new THREE.Mesh(circleGeom, circleMat);
        circle.position.z = 0.1;
        this.scene.add(circle);

        this.animationId = requestAnimationFrame(() => this.animate());
    }

    private animate(): void {
        this.animationId = requestAnimationFrame(() => this.animate());
        this.controls.update();
        this.renderer.render(this.scene, this.camera);
    }

    destroy(): void {
        cancelAnimationFrame(this.animationId);
        this.world.dispose();
        this.resizeObserver.disconnect();
        this.renderer.dispose();
        this.renderer.domElement.remove();
    }
}

export async function createGame(container: HTMLElement): Promise<Game> {
    await init(wasmUrl);
    const game = new Game(container);
    game.init();
    return game;
}
