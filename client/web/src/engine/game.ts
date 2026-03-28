import init from '#wasm';
import wasmUrl from '#wasm-bin';
import * as THREE from 'three';
import { Camera } from '../camera/camera';
import { WorldReferenceSystem } from '../systems/world-reference-system';
import { chunkIdToWorldPosition } from '../world/hex-utils';
import { ChunkId } from '../world/types';
import { World } from '../world/world';
import { DebugPanel } from './debug-panel';
import type { GameSystem } from './game-system';
import { RenderContext } from './render-context';

class Game {
    private readonly events: EventTarget;
    private readonly renderContext: RenderContext;
    private readonly camera: Camera;
    private readonly world: World;
    private readonly debugPanel: DebugPanel;
    private readonly systems: GameSystem[] = [];
    private animationId = 0;
    private lastTime = 0;

    constructor(private readonly container: HTMLElement) {
        this.events = new EventTarget();
        this.renderContext = new RenderContext(container, this.events);
        this.debugPanel = new DebugPanel();
        this.camera = new Camera(this.renderContext, this.events);
        this.world = new World(this.events, this.debugPanel);

        // Add debug toggles
        this.debugPanel.addToggle('Controls', 'Show Center Dot', this.camera, 'showCenterDot');
        this.debugPanel.addToggle('Controls', 'Show Chunk Labels', this.world, 'showChunkLabels');
        this.debugPanel.addButton('Controls', 'Teleport Random', () => this.teleportToRandomChunk());

        // Register systems
        this.systems.push(new WorldReferenceSystem(this.camera, this.world, this.events, this.debugPanel));

        // Add world to scene
        this.renderContext.scene.add(this.world.group);
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
        this.renderContext.scene.add(circle);

        this.lastTime = performance.now();
        this.animationId = requestAnimationFrame((t) => this.animate(t));
    }

    private animate(currentTime: number): void {
        this.animationId = requestAnimationFrame((t) => this.animate(t));

        const deltaTime = (currentTime - this.lastTime) / 1000;
        this.lastTime = currentTime;

        // Update camera
        this.camera.update();

        // Update systems
        for (const system of this.systems) {
            system.update(deltaTime);
        }

        // Render
        this.renderContext.render(this.camera.camera);
    }

    destroy(): void {
        cancelAnimationFrame(this.animationId);
        this.camera.destroy();
        for (const system of this.systems) {
            system.destroy();
        }
        this.world.dispose();
        this.renderContext.destroy();
        this.debugPanel.destroy();
    }

    private teleportToRandomChunk(): void {
        // Generate random chunk coordinates (full i32 range)
        const randomI32 = () => Math.floor(Math.random() * 0xffffffff) - 0x7fffffff;
        const q = randomI32();
        const r = randomI32();
        const targetChunk = new ChunkId(q, r);

        // Get world position of target chunk (relative to current reference)
        const targetPos = chunkIdToWorldPosition(this.world.referenceChunkId, targetChunk);

        // Teleport camera - this updates position, controls, and worldPosition
        this.camera.teleportTo(targetPos);
    }
}

export async function createGame(container: HTMLElement): Promise<Game> {
    await init(wasmUrl);
    const game = new Game(container);
    game.init();
    return game;
}
