import { PixelMapStream } from '#wasm';
import * as THREE from 'three';
import type { ConnectionState, StreamHub, StreamSubscription } from '../../engine/net/stream-hub';
import type { SceneContext } from '../../engine/scene';
import { Experiment } from '../experiment';
import { ClickTracker } from './pixel-map-input';
import { PixelMapUi, statusText } from './pixel-map-ui';
import { PixelMapView } from './pixel-map-view';

const DEFAULT_STREAM_ID = 'pixel-map:pixel-demo';

// A shared pixel map: click a cell to paint it. The map is a command stream of the builder service;
// the replay and pending paints live in the wasm PixelMapStream. The host owns the shared socket;
// this scene owns only its subscription between init() and dispose().
export class PixelMapExperiment extends Experiment {
    private stream: PixelMapStream | null = null;
    private view: PixelMapView | null = null;
    private readonly hub: StreamHub;
    private readonly ui: PixelMapUi;
    private readonly subscription: StreamSubscription;
    private readonly clicks = new ClickTracker();
    private readonly raycaster = new THREE.Raycaster();
    private connectionState: ConnectionState = 'connecting';
    private resubscribeTimer: ReturnType<typeof setTimeout> | null = null;
    private time = 0;

    constructor(context: SceneContext) {
        super(context, { title: 'Pixel Map' });
        if (!context.streamHub) throw new Error('Pixel Map requires a streamHub in the scene config.');
        this.hub = context.streamHub;

        this.camera.position.set(0, -14, 38);
        this.camera.lookAt(0, 0, 0);

        this.ui = new PixelMapUi(
            context.container,
            (count) => this.scatter(count),
            () => this.resync()
        );
        this.subscription = this.hub.createSubscription(DEFAULT_STREAM_ID, (userId) => {
            const stream = new PixelMapStream(DEFAULT_STREAM_ID, userId);
            this.stream = stream;
            this.view = new PixelMapView(this.scene, stream.width(), stream.height());
            return {
                connected: () => {
                    this.connectionState = 'syncing';
                },
                disconnected: () => {
                    this.clearResubscribeTimer();
                    this.connectionState = 'reconnecting';
                },
                frame: (frame) => this.onStreamFrame(stream, frame)
            };
        });

        const canvas = this.renderer.domElement;
        canvas.addEventListener('pointerdown', this.onPointerDown);
        canvas.addEventListener('pointerup', this.onPointerUp);
        canvas.addEventListener('pointercancel', this.onPointerCancel);
    }

    init(): void {
        this.context.runtime.spawn(this.subscription.subscribe());
    }

    protected override onUpdate(dt: number): void {
        this.time += dt;
        if (!this.stream || !this.view) return;
        const live = this.connectionState === 'live' && this.stream.status() === 'live';
        this.view.update(this.stream, !live, this.time, dt);
        const { text, tone } = statusText(
            this.connectionState,
            this.stream.status(),
            this.stream.version(),
            this.stream.pending_count(),
            null
        );
        this.ui.setStatus(text, tone);
    }

    override dispose(): void {
        const canvas = this.renderer.domElement;
        canvas.removeEventListener('pointerdown', this.onPointerDown);
        canvas.removeEventListener('pointerup', this.onPointerUp);
        canvas.removeEventListener('pointercancel', this.onPointerCancel);
        this.clearResubscribeTimer();
        this.subscription.unsubscribe();
        this.view?.dispose();
        this.ui.dispose();
        this.stream?.free();
        super.dispose();
    }

    private onStreamFrame(stream: PixelMapStream, frame: unknown): void {
        for (const reply of stream.handle_message(JSON.stringify(frame))) this.sendFrame(reply);
        const status = stream.status();
        this.connectionState = status === 'live' ? 'live' : 'syncing';
        if (status === 'unsynced' && this.resubscribeTimer === null) {
            this.resubscribeTimer = setTimeout(() => {
                this.resubscribeTimer = null;
                if (stream.status() === 'unsynced') void this.subscription.subscribe();
            }, 5000);
        }
    }

    private sendFrame(text: string): boolean {
        const frame: unknown = JSON.parse(text);
        if (typeof frame !== 'object' || frame === null || !('type' in frame)) {
            throw new Error('Invalid pixel-map client frame');
        }
        if (frame.type === 'streamSubscribe') {
            void this.subscription.subscribe();
            return this.hub.isConnected;
        }
        if (
            frame.type === 'streamCommand' &&
            'commandId' in frame &&
            typeof frame.commandId === 'string' &&
            'command' in frame &&
            typeof frame.command === 'string'
        ) {
            return this.subscription.sendCommand(frame.commandId, frame.command);
        }
        throw new Error('Unsupported pixel-map client frame');
    }

    private clearResubscribeTimer(): void {
        if (this.resubscribeTimer !== null) clearTimeout(this.resubscribeTimer);
        this.resubscribeTimer = null;
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
        if (!this.view) return null;
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
        if (this.stream) this.sendFrame(this.stream.resync());
    }

    // Paints random pixels in random colors (never the erase color), to see many commands at once.
    private scatter(count: number): void {
        if (!this.stream) return;
        const palette = this.stream.palette_size();
        for (let i = 0; i < count; i++) {
            const x = Math.floor(Math.random() * this.stream.width());
            const y = Math.floor(Math.random() * this.stream.height());
            if (!this.paint(x, y, 1 + Math.floor(Math.random() * (palette - 1)))) return;
        }
    }

    private paint(x: number, y: number, color = this.ui.selectedColor): boolean {
        if (!this.stream) return false;
        try {
            const frame = this.stream.paint(x, y, color, crypto.randomUUID());
            // A closed socket keeps the paint pending: the next sync sends it again.
            this.sendFrame(frame);
            return true;
        } catch {
            // Not live: nothing is queued, the status chip already says why.
            return false;
        }
    }
}
