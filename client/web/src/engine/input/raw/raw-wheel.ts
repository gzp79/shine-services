/**
 * RawWheel handles mouse wheel events.
 * Subscribes to wheel events on the provided target.
 */
export class RawWheel {
    onZoom?: (delta: number) => void;

    private _enabled = true;

    constructor(private readonly target: HTMLElement) {
        this.target.addEventListener('wheel', this.handleWheel);
    }

    get enabled(): boolean {
        return this._enabled;
    }

    set enabled(value: boolean) {
        this._enabled = value;
    }

    dispose(): void {
        this.target.removeEventListener('wheel', this.handleWheel);
    }

    private handleWheel = (ev: WheelEvent): void => {
        if (!this.enabled) return;
        ev.preventDefault();
        this.onZoom?.(ev.deltaY);
    };
}
