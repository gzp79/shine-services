import type { Point } from '../input-handler';

export class RawPointerTracker {
    onMove?: (pos: Point) => void;
    onLeave?: () => void;

    private _enabled = true;
    private rect: DOMRect;
    private readonly resizeObserver: ResizeObserver;

    constructor(private readonly target: HTMLElement) {
        this.rect = target.getBoundingClientRect();
        this.resizeObserver = new ResizeObserver(() => {
            this.rect = this.target.getBoundingClientRect();
        });
        this.resizeObserver.observe(target);
        this.target.addEventListener('pointermove', this.handlePointerMove);
        this.target.addEventListener('mouseleave', this.handleMouseLeave);
    }

    get enabled(): boolean {
        return this._enabled;
    }

    set enabled(value: boolean) {
        if (this._enabled === value) return;
        this._enabled = value;
        if (!value) {
            this.onLeave?.();
        }
    }

    dispose(): void {
        this.resizeObserver.disconnect();
        this.target.removeEventListener('pointermove', this.handlePointerMove);
        this.target.removeEventListener('mouseleave', this.handleMouseLeave);
    }

    private handlePointerMove = (ev: PointerEvent): void => {
        if (!this._enabled) return;
        if (ev.pointerType === 'touch') return;
        this.onMove?.({ x: ev.clientX - this.rect.left, y: ev.clientY - this.rect.top });
    };

    private handleMouseLeave = (): void => {
        if (!this._enabled) return;
        this.onLeave?.();
    };
}
