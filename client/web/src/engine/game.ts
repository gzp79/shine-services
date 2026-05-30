import init from '#wasm';
import wasmUrl from '#wasm-bin';
import { WebGPURenderer } from 'three/webgpu';
import { WorldCursor } from '../avatar/world-cursor';
import { CameraFollowCursorSystem } from '../systems/camera-follow-cursor-system';
import { CameraViewportSystem } from '../systems/camera-viewport-system';
import { ClearInputStateSystem } from '../systems/clear-input-state-system';
import { CursorDriveSystem } from '../systems/cursor-drive-system';
import { SelectionSystem } from '../systems/selection-system';
import { WorldReferenceSystem } from '../systems/world-reference-system';
import { World } from '../world/world';
import { Camera } from './camera/camera';
import { DebugPanel } from './debug-panel';
import type { GameSystem } from './game-system';
import { InputManager } from './input/input-manager';
import { InputState } from './input/input-state';
import { PerformanceMetrics } from './performance-metrics';
import { RenderContext } from './render-context';

class Game {
    private readonly events: EventTarget;
    private readonly renderContext: RenderContext;
    private readonly inputManager: InputManager;
    private readonly inputState: InputState;
    private readonly camera: Camera;
    private readonly worldCursor: WorldCursor;
    private readonly world: World;
    private readonly debugPanel: DebugPanel;
    private readonly performanceMetrics: PerformanceMetrics;
    private readonly systems: GameSystem[] = [];
    private animationId = 0;
    private lastTime = 0;

    constructor(
        private readonly container: HTMLElement,
        renderer: WebGPURenderer
    ) {
        this.events = new EventTarget();
        this.renderContext = new RenderContext(container, renderer);
        this.debugPanel = new DebugPanel();
        this.debugPanel.setGameContainer(container);
        this.performanceMetrics = new PerformanceMetrics(this.renderContext.renderer);
        this.camera = new Camera(this.events);

        // Make container focusable so keyboard events work
        if (!container.hasAttribute('tabindex')) {
            container.tabIndex = 0;
        }
        // Remove focus outline (keyboard events only, no visual indication needed)
        container.style.outline = 'none';
        container.focus();

        this.inputState = new InputState();
        this.worldCursor = new WorldCursor(this.renderContext, this.events);
        this.inputManager = new InputManager(this.inputState, container);
        this.world = new World(this.events, this.debugPanel);

        // Add debug toggles
        this.worldCursor.showMesh = true;
        this.debugPanel.addToggle('Controls', 'Show World Cursor', this.worldCursor, 'showMesh');
        this.debugPanel.addToggle('Controls', 'Show Chunk Labels', this.world, 'showChunkLabels');
        this.debugPanel.addToggle('Controls', 'Show Cell Wires', this.world, 'showCellWires');

        // Register systems in execution order
        this.systems.push(new CameraViewportSystem(this.camera, this.renderContext));
        this.systems.push(new CursorDriveSystem(this.worldCursor, this.inputState, this.camera));
        this.systems.push(new CameraFollowCursorSystem(this.camera, this.worldCursor, this.events));
        this.systems.push(new WorldReferenceSystem(this.worldCursor, this.world, this.events, this.debugPanel));
        this.systems.push(new SelectionSystem(this.world, this.inputState, this.camera, this.debugPanel));
        this.systems.push(new ClearInputStateSystem(this.inputState));

        // Add world to scene
        this.renderContext.scene.add(this.world.group);
    }

    init(): void {
        this.lastTime = performance.now();
        this.animationId = requestAnimationFrame(() => void this.animate());
    }

    private async animate(): Promise<void> {
        const now = performance.now();
        const deltaTime = (now - this.lastTime) / 1000;
        this.lastTime = now;

        for (const system of this.systems) {
            system.update(deltaTime);
        }

        await this.renderContext.render(this.camera.camera);
        this.performanceMetrics.update(deltaTime);
        this.animationId = requestAnimationFrame(() => void this.animate());
    }

    dispose(): void {
        cancelAnimationFrame(this.animationId);
        this.inputManager.dispose();
        this.camera.dispose();
        this.worldCursor.dispose();
        for (const system of this.systems) {
            system.dispose();
        }
        this.world.dispose();
        this.renderContext.dispose();
        this.debugPanel.dispose();
        this.performanceMetrics.dispose();
    }
}

export async function createGame(container: HTMLElement, renderer: WebGPURenderer): Promise<Game> {
    await init(wasmUrl);
    const game = new Game(container, renderer);
    game.init();
    return game;
}
