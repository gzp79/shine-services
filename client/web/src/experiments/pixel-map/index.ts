import { PixelMapStream } from '#wasm';
import * as THREE from 'three';
import { StreamConnection } from '../../engine/net/stream-connection';
import type { ConnectionState } from '../../engine/net/stream-connection';
import type { SceneContext } from '../../engine/scene';
import { Experiment } from '../experiment';
import { resolveConfig, userIdFromCookie } from './pixel-map-config';
import { ClickTracker } from './pixel-map-input';
import { PixelMapUi, statusText } from './pixel-map-ui';
import { PixelMapView } from './pixel-map-view';

// A shared pixel map: click a cell to paint it. The map is a command stream of the builder service;
// the replay and the pending (unconfirmed) paints live in the wasm PixelMapStream, the socket in a
// StreamConnection that exists only between init() and dispose().
export class PixelMapExperiment extends Experiment {
    private readonly stream: PixelMapStream;
    private readonly view: PixelMapView;
    private readonly ui: PixelMapUi;
    private readonly connection: StreamConnection;
    private readonly clicks = new ClickTracker();
    private readonly raycaster = new THREE.Raycaster();
    private connectionState: ConnectionState = 'connecting';
    private time = 0;

    constructor(context: SceneContext) {
        super(context, { title: 'Pixel Map' });
        const config = resolveConfig(new URLSearchParams(window.location.search));

        this.camera.position.set(0, -14, 38);
        this.camera.lookAt(0, 0, 0);

        const userId = userIdFromCookie(document.cookie);
        this.stream = new PixelMapStream(config.streamId, userId ?? crypto.randomUUID());
        this.view = new PixelMapView(this.scene, this.stream.width(), this.stream.height());
        this.ui = new PixelMapUi(
            context.container,
            (count) => this.scatter(count),
            () => this.resync()
        );
        this.ui.setUser(userId);
        this.connection = new StreamConnection({
            url: config.wsUrl,
            stream: {
                subscribeFrame: () => this.stream.subscribe_frame(),
                handleMessage: (text) => this.stream.handle_message(text),
                status: () => this.stream.status()
            },
            onState: (state) => {
                this.connectionState = state;
            },
            onChange: () => {}
        });

        const canvas = this.renderer.domElement;
        canvas.addEventListener('pointerdown', this.onPointerDown);
        canvas.addEventListener('pointerup', this.onPointerUp);
        canvas.addEventListener('pointercancel', this.onPointerCancel);
    }

    init(): void {
        this.connection.start();
    }

    protected override onUpdate(dt: number): void {
        this.time += dt;
        const live = this.connectionState === 'live' && this.stream.status() === 'live';
        this.view.update(this.stream, !live, this.time, dt);
        const { text, tone } = statusText(
            this.connectionState,
            this.stream.status(),
            this.stream.version(),
            this.stream.pending_count()
        );
        this.ui.setStatus(text, tone);
    }

    override dispose(): void {
        const canvas = this.renderer.domElement;
        canvas.removeEventListener('pointerdown', this.onPointerDown);
        canvas.removeEventListener('pointerup', this.onPointerUp);
        canvas.removeEventListener('pointercancel', this.onPointerCancel);
        this.connection.stop();
        this.view.dispose();
        this.ui.dispose();
        this.stream.free();
        super.dispose();
    }

    private onPointerDown = (event: PointerEvent): void => {
        // A second finger makes it a pinch or pan, never a click.
        if (!event.isPrimary) {
            this.clicks.cancel();
            return;
        }
        this.clicks.down(event.clientX, event.clientY, event.timeStamp);
    };

    private onPointerCancel = (): void => {
        this.clicks.cancel();
    };

    private onPointerUp = (event: PointerEvent): void => {
        if (!event.isPrimary || !this.clicks.up(event.clientX, event.clientY, event.timeStamp)) return;
        const cell = this.pickCell(event);
        if (cell) this.paint(cell.x, cell.y);
    };

    private pickCell(event: PointerEvent): { x: number; y: number } | null {
        const rect = this.renderer.domElement.getBoundingClientRect();
        const ndc = new THREE.Vector2(
            ((event.clientX - rect.left) / rect.width) * 2 - 1,
            -(((event.clientY - rect.top) / rect.height) * 2 - 1)
        );
        this.raycaster.setFromCamera(ndc, this.camera);
        const hit = this.raycaster.intersectObject(this.view.pickTarget)[0];
        return hit ? this.view.cellAt(hit.point) : null;
    }

    // Subscribes again, as after a connect: the server sends the state and the pending paints go out again.
    private resync(): void {
        this.connection.send(this.stream.resync());
    }

    // Paints random pixels in random colors (never the erase color), to see many commands at once.
    private scatter(count: number): void {
        const palette = this.stream.palette_size();
        for (let i = 0; i < count; i++) {
            const x = Math.floor(Math.random() * this.stream.width());
            const y = Math.floor(Math.random() * this.stream.height());
            if (!this.paint(x, y, 1 + Math.floor(Math.random() * (palette - 1)))) return;
        }
    }

    private paint(x: number, y: number, color = this.ui.selectedColor): boolean {
        try {
            const frame = this.stream.paint(x, y, color, crypto.randomUUID());
            // A closed socket keeps the paint pending: the next sync sends it again.
            this.connection.send(frame);
            return true;
        } catch {
            // Not live: nothing is queued, the status chip already says why.
            return false;
        }
    }
}
