import init, { WasmWorld } from '#wasm';
import wasmUrl from '#wasm-bin';
import * as THREE from 'three';
import { SceneContext, animate, createScene } from './scene';
import { ChunkId } from './world/types';

interface ChunkObject {
    group: THREE.Group;
    dispose(): void;
}

class Game {
    private readonly world: WasmWorld;
    private readonly ctx: SceneContext;
    private readonly chunks = new Map<string, ChunkObject>();
    private animationId = 0;

    constructor(container: HTMLElement) {
        this.ctx = createScene(container);
        this.world = new WasmWorld();
    }

    init(): void {
        this.loadChunk(new ChunkId(0, 0));
        this.animationId = animate(this.ctx);
    }

    loadChunk(id: ChunkId): void {
        const key = id.key();
        if (this.chunks.has(key)) return;

        this.world.init_chunk(id.q, id.r);

        const group = new THREE.Group();
        this.ctx.scene.add(group);
        this.chunks.set(key, {
            group,
            dispose: () => this.ctx.scene.remove(group)
        });
    }

    unloadChunk(id: ChunkId): void {
        const key = id.key();
        const obj = this.chunks.get(key);
        if (!obj) return;
        obj.dispose();
        this.chunks.delete(key);
    }

    destroy(): void {
        cancelAnimationFrame(this.animationId);
        for (const obj of this.chunks.values()) {
            obj.dispose();
        }
        this.chunks.clear();
        this.world.free();
        this.ctx.resizeObserver.disconnect();
        this.ctx.renderer.dispose();
        this.ctx.renderer.domElement.remove();
    }
}

export async function createGame(container: HTMLElement): Promise<Game> {
    await init(wasmUrl);
    const game = new Game(container);
    game.init();
    return game;
}
