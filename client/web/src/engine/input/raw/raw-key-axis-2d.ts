export interface KeyAxis2DMapping {
    up: string;
    down: string;
    left: string;
    right: string;
    sprint?: string;
}

/**
 * Generic 2D axis from keyboard keys (e.g., WASD, arrow keys, etc.)
 * Subscribes to keyboard events on the provided target.
 */
export class RawKeyAxis2D {
    onStart?: () => void;
    onEnd?: () => void;
    onChange?: (x: number, y: number, sprint: boolean) => void;

    private _enabled = true;
    private active = false;
    private keys = { up: false, down: false, left: false, right: false, sprint: false };

    constructor(
        private readonly mapping: KeyAxis2DMapping,
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
            this.onChange?.(0, 0, false);
            this.onEnd?.();
        }
    }

    isActive(): boolean {
        return this.keys.up || this.keys.down || this.keys.left || this.keys.right;
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

        if (k === this.mapping.up.toLowerCase()) this.keys.up = true;
        else if (k === this.mapping.down.toLowerCase()) this.keys.down = true;
        else if (k === this.mapping.left.toLowerCase()) this.keys.left = true;
        else if (k === this.mapping.right.toLowerCase()) this.keys.right = true;
        else if (this.mapping.sprint && k === this.mapping.sprint.toLowerCase()) this.keys.sprint = true;
        else return;

        ev.preventDefault();

        if (!wasActive && this.isActive()) {
            this.active = true;
            this.onStart?.();
        }

        const axis = this.getAxis();
        this.onChange?.(axis.x, axis.y, this.keys.sprint);
    };

    private handleKeyUp = (ev: Event): void => {
        if (!(ev instanceof KeyboardEvent)) return;
        const k = ev.key.toLowerCase();

        if (k === this.mapping.up.toLowerCase()) this.keys.up = false;
        else if (k === this.mapping.down.toLowerCase()) this.keys.down = false;
        else if (k === this.mapping.left.toLowerCase()) this.keys.left = false;
        else if (k === this.mapping.right.toLowerCase()) this.keys.right = false;
        else if (this.mapping.sprint && k === this.mapping.sprint.toLowerCase()) this.keys.sprint = false;
        else return;

        ev.preventDefault();

        const axis = this.getAxis();
        this.onChange?.(axis.x, axis.y, this.keys.sprint);

        if (this.active && !this.isActive()) {
            this.active = false;
            this.onEnd?.();
        }
    };

    private getAxis(): { x: number; y: number } {
        let x = 0;
        let y = 0;
        if (this.keys.left) x -= 1;
        if (this.keys.right) x += 1;
        if (this.keys.up) y += 1;
        if (this.keys.down) y -= 1;
        return { x, y };
    }
}
