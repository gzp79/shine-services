export interface KeyAxis1DMapping {
    negative: string; // e.g., 'Q', 'R'
    positive: string; // e.g., 'E', 'F'
}

/**
 * Generic 1D axis from keyboard keys (e.g., Q/E rotation, R/F zoom, etc.)
 * Subscribes to keyboard events on the provided target.
 */
export class RawKeyAxis1D {
    onStart?: () => void;
    onEnd?: () => void;
    onChange?: (value: number) => void;

    private _enabled = true;
    private active = false;
    private keys = { negative: false, positive: false };

    constructor(
        private readonly mapping: KeyAxis1DMapping,
        private readonly target: EventTarget = window
    ) {
        this.target.addEventListener('keydown', this.handleKeyDown);
        this.target.addEventListener('keyup', this.handleKeyUp);
    }

    get enabled(): boolean {
        return this._enabled;
    }

    set enabled(value: boolean) {
        if (this._enabled === value) return;

        const wasActive = this.active;
        this._enabled = value;

        if (!value && wasActive) {
            this.active = false;
            this.onChange?.(0);
            this.onEnd?.();
        }
    }

    isActive(): boolean {
        return this.keys.negative || this.keys.positive;
    }

    dispose(): void {
        this.target.removeEventListener('keydown', this.handleKeyDown);
        this.target.removeEventListener('keyup', this.handleKeyUp);
    }

    private handleKeyDown = (ev: Event): void => {
        if (!(ev instanceof KeyboardEvent)) return;
        if (!this.enabled) return;

        const k = ev.key.toLowerCase();
        const wasActive = this.active;

        if (k === this.mapping.negative.toLowerCase()) this.keys.negative = true;
        else if (k === this.mapping.positive.toLowerCase()) this.keys.positive = true;
        else return;

        ev.preventDefault();

        if (!wasActive && this.isActive()) {
            this.active = true;
            this.onStart?.();
        }

        this.onChange?.(this.getValue());
    };

    private handleKeyUp = (ev: Event): void => {
        if (!(ev instanceof KeyboardEvent)) return;
        const k = ev.key.toLowerCase();

        if (k === this.mapping.negative.toLowerCase()) this.keys.negative = false;
        else if (k === this.mapping.positive.toLowerCase()) this.keys.positive = false;
        else return;

        ev.preventDefault();

        this.onChange?.(this.getValue());

        if (this.active && !this.isActive()) {
            this.active = false;
            this.onEnd?.();
        }
    };

    private getValue(): number {
        let value = 0;
        if (this.keys.negative) value -= 1;
        if (this.keys.positive) value += 1;
        return value;
    }
}
