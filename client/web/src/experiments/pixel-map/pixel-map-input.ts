import { InputConst } from '../../constants';

// Tells a click from the start of a camera drag, so painting does not fight the orbit controls.
export class ClickTracker {
    private start: { x: number; y: number; time: number } | null = null;

    down(x: number, y: number, time: number): void {
        this.start = { x, y, time };
    }

    cancel(): void {
        this.start = null;
    }

    // True if this release ends a click: short, and the pointer stayed in place.
    up(x: number, y: number, time: number): boolean {
        const start = this.start;
        this.start = null;
        if (!start) return false;
        const moved = Math.hypot(x - start.x, y - start.y);
        return moved <= InputConst.MOVE_THRESHOLD_PX && time - start.time <= InputConst.TAP_THRESHOLD_MS;
    }
}
