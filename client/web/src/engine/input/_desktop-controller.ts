import {
    LONG_PRESS_MS,
    MOVE_KEYS,
    MOVE_KEY_DOWN,
    MOVE_KEY_LEFT,
    MOVE_KEY_RIGHT,
    MOVE_KEY_SPRINT,
    MOVE_KEY_UP,
    MOVE_THRESHOLD_PX,
    TAP_THRESHOLD_MS
} from '../../constants';
import { PointerInfo } from './_input-controller';
import type { Delta, InputHandler, Point } from './_input-handler';

// Maps button index to buttons bitmask
// button property: 0=left, 1=middle, 2=right
// buttons bitmask: bit0(1)=left, bit1(2)=right, bit2(4)=middle
const BUTTON_TO_BITMASK = [1, 4, 2];

export class DesktopController {
    private owningPointer: PointerInfo | null = null;
    private isOwningPointerDead = false;
    private keys = { up: false, left: false, down: false, right: false, shift: false };
    private activePan = false;
    private activeRotate = false;
    private activeInteract = false;
    private longPressTimer: number | null = null;
    private prevDirection = { x: 0, y: 0 };
    private prevShift = false;
    private handler: InputHandler;

    constructor(handler: InputHandler) {
        this.handler = handler;
    }

    handlePointerDown(ev: PointerEvent): void {
        // Filter middle button
        if (ev.button === 1) return;

        // Dead pointers cannot start new gestures
        if (this.isOwningPointerDead) {
            return;
        }

        // First button down owns the gesture - ignore additional buttons
        if (this.owningPointer !== null) {
            return;
        }

        const p = this.pos(ev);
        this.owningPointer = {
            id: ev.pointerId,
            startPos: p,
            currentPos: p,
            downTime: performance.now(),
            button: ev.button
        };
        this.isOwningPointerDead = false;

        // Left button only - start long press timer
        if (ev.button === 0 && !this.activeInteract) {
            this.activePan = false;
            this.activeRotate = false;
            this.cancelLongPressTimer();

            this.longPressTimer = window.setTimeout(() => {
                if (this.owningPointer !== null && !this.activePan && !this.activeRotate) {
                    this.activeInteract = true;
                    this.handler.onInteractStart(p);
                }
            }, LONG_PRESS_MS);
        }
    }

    handlePointerMove(ev: PointerEvent): void {
        // Only handle the owning pointer
        if (this.owningPointer === null || ev.pointerId !== this.owningPointer.id) {
            return;
        }

        // Dead pointers cannot interact
        if (this.isOwningPointerDead) {
            return;
        }

        // Check if owning button was released (multiple buttons can be down, but pointerup only fires when ALL are released)
        const owningButtonMask = BUTTON_TO_BITMASK[this.owningPointer.button];
        if (owningButtonMask && (ev.buttons & owningButtonMask) === 0) {
            // Owning button released - end active gesture
            this.endActiveGesture(this.owningPointer.currentPos);
            return;
        }

        const newPos = this.pos(ev);
        this.owningPointer.currentPos = newPos;
        const totalMoved = this.dist(this.owningPointer.startPos, newPos);

        // Cancel long press timer if moved beyond threshold
        if (totalMoved > MOVE_THRESHOLD_PX) {
            this.cancelLongPressTimer();
        }

        // Interact drag (after long press)
        if (this.activeInteract) {
            this.handler.onInteractDrag(this.owningPointer.startPos, newPos);
            return;
        }

        // Pan (left button drag)
        if (
            this.owningPointer.button === 0 &&
            !this.activePan &&
            !this.activeRotate &&
            totalMoved > MOVE_THRESHOLD_PX
        ) {
            // Check if move is active - block pan
            if (this.keys.up || this.keys.left || this.keys.down || this.keys.right) {
                return;
            }
            this.activePan = true;
            this.handler.onPanStart(this.owningPointer.startPos);
        }

        if (this.activePan) {
            this.handler.onPan(this.owningPointer.startPos, newPos);
            return;
        }

        // Rotate (right button drag)
        if (
            this.owningPointer.button === 2 &&
            !this.activePan &&
            !this.activeRotate &&
            totalMoved > MOVE_THRESHOLD_PX
        ) {
            this.activeRotate = true;
            this.handler.onRotateStart(this.owningPointer.startPos);
        }

        if (this.activeRotate) {
            this.handler.onRotate(this.owningPointer.startPos, newPos);
        }
    }

    handlePointerUp(ev: PointerEvent): void {
        // Only handle the owning pointer with the owning button
        // Other buttons pressed/released on the same pointer are ignored
        if (
            this.owningPointer === null ||
            ev.pointerId !== this.owningPointer.id ||
            ev.button !== this.owningPointer.button
        ) {
            return;
        }

        // Handle dead pointers
        if (this.isOwningPointerDead) {
            this.isOwningPointerDead = false;
            this.owningPointer = null;
            return;
        }

        // End active gesture (handles tap detection too)
        this.endActiveGesture(this.owningPointer.currentPos);
    }

    handlePointerCancel(ev: PointerEvent): void {
        // Only handle the owning pointer with the owning button
        // Other buttons pressed/released on the same pointer are ignored
        if (
            this.owningPointer === null ||
            ev.pointerId !== this.owningPointer.id ||
            ev.button !== this.owningPointer.button
        ) {
            return;
        }

        // Handle dead pointers
        if (this.isOwningPointerDead) {
            this.isOwningPointerDead = false;
            this.owningPointer = null;
            return;
        }

        // End active gesture
        this.endActiveGesture(this.owningPointer.currentPos);
    }

    handleKeyDown(ev: KeyboardEvent): void {
        const key = ev.key.toLowerCase();
        if (!MOVE_KEYS.includes(key)) {
            return;
        }

        ev.preventDefault();

        // Block move during interact or pan (but allow during rotate for FPS camera)
        if (this.activeInteract || this.activePan) {
            return;
        }

        // Cancel tap/interact timers and mark pointer as dead
        if (this.longPressTimer !== null) {
            this.cancelLongPressTimer();
            if (this.owningPointer !== null) {
                this.isOwningPointerDead = true;
            }
        }

        // Update key state
        if (key === MOVE_KEY_SPRINT) this.keys.shift = true;
        if (key === MOVE_KEY_UP) this.keys.up = true;
        if (key === MOVE_KEY_LEFT) this.keys.left = true;
        if (key === MOVE_KEY_DOWN) this.keys.down = true;
        if (key === MOVE_KEY_RIGHT) this.keys.right = true;

        // Emit move event (only if direction changed)
        this.emitMove();
    }

    handleKeyUp(ev: KeyboardEvent): void {
        const key = ev.key.toLowerCase();
        if (!MOVE_KEYS.includes(key)) {
            return;
        }

        ev.preventDefault();

        // Update key state
        if (key === MOVE_KEY_SPRINT) this.keys.shift = false;
        if (key === MOVE_KEY_UP) this.keys.up = false;
        if (key === MOVE_KEY_LEFT) this.keys.left = false;
        if (key === MOVE_KEY_DOWN) this.keys.down = false;
        if (key === MOVE_KEY_RIGHT) this.keys.right = false;

        // Emit move event (may be zero vector)
        this.emitMove();
    }

    handleWheel(ev: WheelEvent): void {
        // Block wheel only during interact (allow with pan/rotate for camera control)
        if (this.activeInteract) {
            return;
        }

        // Cancel tap/interact timers and mark pointer as dead
        if (this.longPressTimer !== null) {
            this.cancelLongPressTimer();
            if (this.owningPointer !== null) {
                this.isOwningPointerDead = true;
            }
        }

        const pos: Point = { x: ev.clientX, y: ev.clientY };
        this.handler.onZoom(pos, ev.deltaY);
    }

    reset(): void {
        // Emit END events for active gestures
        if (this.activeInteract && this.owningPointer) {
            this.handler.onInteractEnd(this.owningPointer.currentPos);
        } else if (this.activePan && this.owningPointer) {
            this.handler.onPanEnd(this.owningPointer.currentPos);
        } else if (this.activeRotate && this.owningPointer) {
            this.handler.onRotateEnd(this.owningPointer.currentPos);
        }

        // Emit move event with zero direction if keys were pressed
        if (this.keys.up || this.keys.left || this.keys.down || this.keys.right) {
            this.keys = { up: false, left: false, down: false, right: false, shift: false };
            this.emitMove();
        }

        // Clear all state
        this.owningPointer = null;
        this.isOwningPointerDead = false;
        this.activePan = false;
        this.activeRotate = false;
        this.activeInteract = false;
        this.prevDirection = { x: 0, y: 0 };
        this.prevShift = false;
        this.cancelLongPressTimer();
    }

    private dist(a: Point, b: Point): number {
        return Math.hypot(a.x - b.x, a.y - b.y);
    }

    private pos(ev: PointerEvent): Point {
        return { x: ev.clientX, y: ev.clientY };
    }

    private cancelLongPressTimer(): void {
        if (this.longPressTimer !== null) {
            clearTimeout(this.longPressTimer);
            this.longPressTimer = null;
        }
    }

    private calculateMoveDirection(): Delta {
        let x = 0,
            y = 0;
        if (this.keys.left) x -= 1;
        if (this.keys.right) x += 1;
        if (this.keys.up) y -= 1;
        if (this.keys.down) y += 1;

        if (x !== 0 || y !== 0) {
            const len = Math.hypot(x, y);
            x /= len;
            y /= len;
        }

        return { x, y };
    }

    private emitMove(): void {
        const direction = this.calculateMoveDirection();

        // Only emit if direction changed
        if (
            direction.x !== this.prevDirection.x ||
            direction.y !== this.prevDirection.y ||
            this.keys.shift !== this.prevShift
        ) {
            this.prevDirection = direction;
            this.prevShift = this.keys.shift;
            this.handler.onMove(direction, this.keys.shift);
        }
    }

    private endActiveGesture(releasedPos: Point): void {
        // Check for tap if we have pointer data
        let isTap = false;
        if (this.owningPointer) {
            const duration = performance.now() - this.owningPointer.downTime;
            const totalMoved = this.dist(this.owningPointer.startPos, this.owningPointer.currentPos);
            isTap =
                this.owningPointer.button === 0 &&
                !this.activePan &&
                !this.activeInteract &&
                !this.activeRotate &&
                duration < TAP_THRESHOLD_MS &&
                totalMoved <= MOVE_THRESHOLD_PX;
        }

        // Clear pointer state
        this.owningPointer = null;
        this.isOwningPointerDead = false;
        this.cancelLongPressTimer();

        // End active gestures (defensive: clear even if owningPointer was somehow null)
        if (this.activeInteract) {
            this.activeInteract = false;
            this.handler.onInteractEnd(releasedPos);
        } else if (this.activePan) {
            this.activePan = false;
            this.handler.onPanEnd(releasedPos);
        } else if (this.activeRotate) {
            this.activeRotate = false;
            this.handler.onRotateEnd(releasedPos);
        }

        // Emit tap if qualified
        if (isTap) {
            this.handler.onTap(releasedPos);
        }
    }

    isActive(): boolean {
        return this.owningPointer !== null || this.keys.up || this.keys.left || this.keys.down || this.keys.right;
    }
}
