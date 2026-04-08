import init from '#wasm';
import wasmUrl from '#wasm-bin';
import { InputMapper } from '../avatar/input-mapper';
import { WorldCursor } from '../avatar/world-cursor';
import { CameraFollowCursorSystem } from '../systems/camera-follow-cursor-system';
import { ChunkHoverSystem } from '../systems/chunk-hover-system';
import { WorldReferenceSystem } from '../systems/world-reference-system';
import { World } from '../world/world';
import { Camera } from './camera/camera';
import { DebugPanel } from './debug-panel';
import type { GameSystem } from './game-system';
import { InputController } from './input';
import { PerformanceMetrics } from './performance-metrics';
import { RenderContext } from './render-context';

class Game {
    private readonly events: EventTarget;
    private readonly renderContext: RenderContext;
    private readonly inputController: InputController;
    private readonly inputMapper: InputMapper;
    private readonly camera: Camera;
    private readonly worldCursor: WorldCursor;
    private readonly world: World;
    private readonly debugPanel: DebugPanel;
    private readonly performanceMetrics: PerformanceMetrics;
    private readonly systems: GameSystem[] = [];
    private animationId = 0;
    private lastTime = 0;

    constructor(private readonly container: HTMLElement) {
        this.events = new EventTarget();
        this.renderContext = new RenderContext(container, this.events);
        this.debugPanel = new DebugPanel();
        this.debugPanel.setGameContainer(container);
        this.performanceMetrics = new PerformanceMetrics(this.renderContext.renderer);
        this.camera = new Camera(this.renderContext, this.events);

        // Make container focusable so keyboard events work
        if (!container.hasAttribute('tabindex')) {
            container.tabIndex = 0;
        }
        // Remove focus outline (keyboard events only, no visual indication needed)
        container.style.outline = 'none';
        container.focus();

        // Create WorldCursor first (needed by InputMapper for orientation)
        this.worldCursor = new WorldCursor(this.renderContext, this.events);
        this.world = new World(this.events, this.debugPanel);

        // InputMapper transforms screen-space input to cursor-aware events
        this.inputMapper = new InputMapper(this.camera, this.worldCursor, this.events);
        this.inputController = new InputController(container, this.inputMapper);

        // Add debug toggles
        this.worldCursor.showMesh = true;
        this.debugPanel.addToggle('Controls', 'Show World Cursor', this.worldCursor, 'showMesh');
        this.debugPanel.addToggle('Controls', 'Show Chunk Labels', this.world, 'showChunkLabels');
        this.debugPanel.addToggle('Controls', 'Show Polygon Wire', this.world, 'showPolygonWire');

        // Register systems
        this.systems.push(new CameraFollowCursorSystem(this.camera, this.worldCursor, this.events));
        this.systems.push(new WorldReferenceSystem(this.worldCursor, this.world, this.events, this.debugPanel));
        this.systems.push(new ChunkHoverSystem(this.world, this.renderContext, this.camera));

        // Add world to scene
        this.renderContext.scene.add(this.world.group);
    }

    init(): void {
        this.lastTime = performance.now();
        this.animationId = requestAnimationFrame((t) => this.animate(t));
    }

    private animate(currentTime: number): void {
        this.animationId = requestAnimationFrame((t) => this.animate(t));
        const deltaTime = (currentTime - this.lastTime) / 1000;
        this.lastTime = currentTime;

        this.camera.update();
        this.worldCursor.update(deltaTime);

        for (const system of this.systems) {
            system.update(deltaTime);
        }

        this.renderContext.render(this.camera.camera);
        this.performanceMetrics.update(deltaTime);
    }

    dispose(): void {
        cancelAnimationFrame(this.animationId);
        this.inputController.dispose();
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

export async function createGame(container: HTMLElement): Promise<Game> {
    await init(wasmUrl);
    const game = new Game(container);
    game.init();
    return game;
}
