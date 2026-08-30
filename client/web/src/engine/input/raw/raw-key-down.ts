/**
 * Fires onDown once per physical key press (key repeat ignored).
 */
export class RawKeyDown {
    onDown?: () => void;

    private _enabled = true;

    constructor(
        private readonly key: string,
        private readonly target: EventTarget = window
    ) {
        this.target.addEventListener('keydown', this.handleKeyDown);
    }

    get enabled(): boolean {
        return this._enabled;
    }
    set enabled(value: boolean) {
        this._enabled = value;
    }

    dispose(): void {
        this.target.removeEventListener('keydown', this.handleKeyDown);
    }

    private handleKeyDown = (ev: Event): void => {
        if (!(ev instanceof KeyboardEvent)) return;
        if (!this._enabled) return;
        if (ev.repeat) return;
        if (ev.key.toLowerCase() === this.key.toLowerCase()) {
            ev.preventDefault();
            this.onDown?.();
        }
    };
}
