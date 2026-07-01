/**
 * Fires onDown once per physical key press (key repeat ignored).
 */
export class RawKeyDown {
    onDown?: () => void;

    constructor(
        private readonly key: string,
        private readonly target: EventTarget = window
    ) {
        this.target.addEventListener('keydown', this.handleKeyDown);
    }

    dispose(): void {
        this.target.removeEventListener('keydown', this.handleKeyDown);
    }

    private handleKeyDown = (ev: Event): void => {
        if (!(ev instanceof KeyboardEvent)) return;
        if (ev.repeat) return;
        if (ev.key.toLowerCase() === this.key.toLowerCase()) {
            ev.preventDefault();
            this.onDown?.();
        }
    };
}
