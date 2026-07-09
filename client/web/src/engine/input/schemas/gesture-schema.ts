import type { InputHandler, Point } from '../input-handler';
import { RawKeyDown } from '../raw/raw-key-down';
import { RawPointer } from '../raw/raw-pointer';
import { RawSingleTouch } from '../raw/raw-single-touch';
import { InputSchema } from './input-schema';

const MAX_POINTS = 1000;

export class GestureSchema extends InputSchema {
    private readonly toggle: RawKeyDown;
    private readonly pointer: RawPointer;
    private readonly touch: RawSingleTouch;
    private readonly container: HTMLElement;

    // true = schema slot is held (Space #1); false = released (Space #2)
    private _isIdle = true;
    // interleaved normalized [x0, y0, x1, y1, ...] — NDC: center=(0,0), x right, y up, range [-1,1]
    private readonly buf = new Float32Array(MAX_POINTS * 2);
    private count = 0;
    private overflow = false;

    constructor(container: HTMLElement, handler?: InputHandler) {
        super('gesture', handler);
        this.container = container;

        this.toggle = new RawKeyDown(' ');
        this.toggle.onDown = () => {
            if (this.activate()) this._isIdle = !this._isIdle;
            if (this._isIdle) this.cancelGesture();
        };

        this.pointer = new RawPointer(0, false, container);
        this.touch = new RawSingleTouch(container);

        const onPointerStart = (pos: Point) => {
            if (this._isIdle) return;
            this.touch.enabled = false;
            this.count = 0;
            this.overflow = false;
            this.pushPoint(pos);
        };

        const onTouchStart = (pos: Point) => {
            if (this._isIdle) return;
            this.pointer.enabled = false;
            this.count = 0;
            this.overflow = false;
            this.pushPoint(pos);
        };

        const onMove = (current: Point) => {
            if (this._isIdle || this.overflow) return;
            this.pushPoint(current);
        };

        const onEnd = () => {
            if (this._isIdle) return;
            if (!this.overflow && this.count > 0) {
                this.handler?.onGesture(this.buf.subarray(0, this.count * 2));
            }
            this.count = 0;
            this.overflow = false;
            this.touch.enabled = true;
            this.pointer.enabled = true;
        };

        this.pointer.onDragStart = onPointerStart;
        this.pointer.onDrag = (_start, _prev, current) => {
            onMove(current);
        };
        this.pointer.onDragEnd = (_start, end) => {
            onMove(end);
            onEnd();
        };

        this.touch.onDragStart = onTouchStart;
        this.touch.onDrag = (_start, _prev, current) => {
            onMove(current);
        };
        this.touch.onDragEnd = (_start, end) => {
            onMove(end);
            onEnd();
        };
    }

    get currentPoints(): { buf: Float32Array; count: number } {
        return { buf: this.buf, count: this.count };
    }

    private pushPoint(p: Point): void {
        if (this.count >= MAX_POINTS) {
            this.overflow = true;
            return;
        }
        const w = this.container.clientWidth;
        const h = this.container.clientHeight;
        this.buf[this.count * 2] = (p.x / w) * 2 - 1;
        this.buf[this.count * 2 + 1] = -(p.y / h) * 2 + 1;
        this.count++;
    }

    private cancelGesture(): void {
        this.count = 0;
        this.overflow = false;
        this.pointer.cancel();
        this.touch.cancel();
        this.pointer.enabled = true;
        this.touch.enabled = true;
    }

    get isIdle(): boolean {
        return this._isIdle;
    }

    state(): Record<string, string> {
        return {
            enabled: this._isIdle ? 'off' : 'on',
            points: this.overflow ? `${MAX_POINTS}+ (overflow)` : `${this.count}`
        };
    }

    cancel(): void {
        this._isIdle = true;
        this.cancelGesture();
    }

    dispose(): void {
        this.toggle.dispose();
        this.pointer.dispose();
        this.touch.dispose();
    }
}
