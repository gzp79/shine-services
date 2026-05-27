import {
    LONG_PRESS_MS,
    MOVE_KEYS,
    MOVE_KEY_DOWN,
    MOVE_KEY_LEFT,
    MOVE_KEY_RIGHT,
    MOVE_KEY_SPRINT,
    MOVE_KEY_UP,
    MOVE_THRESHOLD_PX,
    ROTATE_KEYS,
    ROTATE_KEY_LEFT,
    ROTATE_KEY_RIGHT,
    TAP_THRESHOLD_MS,
    ZOOM_KEYS,
    ZOOM_KEY_IN,
    ZOOM_KEY_OUT
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
    private rotateKeys = { left: false, right: false };
    private zoomKeys = { in: false, out: false };
    private activeDragPan = false;
    private activeDragRotate = false;
    private activeInteract = false;
    private longPressTimer: number | null = null;
    private prevDirection = { x: 0, y: 0 };
    private prevShift = false;
    private prevRotateDirection = 0;
    private prevZoomDirection = 0;
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
            this.activeDragPan = false;
            this.activeDragRotate = false;
            this.cancelLongPressTimer();

            this.longPressTimer = window.setTimeout(() => {
                if (this.owningPointer !== null && !this.activeDragPan && !this.activeDragRotate) {
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

        // DragPan (left button drag)
        if (
            this.owningPointer.button === 0 &&
            !this.activeDragPan &&
            !this.activeDragRotate &&
            totalMoved > MOVE_THRESHOLD_PX
        ) {
            // Conflict resolution now in InputManager
            this.activeDragPan = true;
            this.handler.onDragPanStart(this.owningPointer.startPos);
        }

        if (this.activeDragPan) {
            this.handler.onDragPan(this.owningPointer.startPos, newPos);
            return;
        }

        // DragRotate (right button drag)
        if (
            this.owningPointer.button === 2 &&
            !this.activeDragPan &&
            !this.activeDragRotate &&
            totalMoved > MOVE_THRESHOLD_PX
        ) {
            // Conflict resolution now in InputManager
            this.activeDragRotate = true;
            this.handler.onDragRotateStart(this.owningPointer.startPos);
        }

        if (this.activeDragRotate) {
            this.handler.onDragRotate(this.owningPointer.startPos, newPos);
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
        const isMoveKey = MOVE_KEYS.includes(key);
        const isRotateKey = ROTATE_KEYS.includes(key);
        const isZoomKey = ZOOM_KEYS.includes(key);

        if (!isMoveKey && !isRotateKey && !isZoomKey) {
            return;
        }

        ev.preventDefault();

        // Block move/rotate during interact or pan
        if (this.activeInteract || this.activeDragPan) {
            return;
        }

        // Block Q/E rotation during drag-rotate
        if (isRotateKey && this.activeDragRotate) {
            return;
        }

        // Cancel tap/interact timers and mark pointer as dead
        if (this.longPressTimer !== null) {
            this.cancelLongPressTimer();
            if (this.owningPointer !== null) {
                this.isOwningPointerDead = true;
            }
        }

        // Update move key state
        if (key === MOVE_KEY_SPRINT) this.keys.shift = true;
        if (key === MOVE_KEY_UP) this.keys.up = true;
        if (key === MOVE_KEY_LEFT) this.keys.left = true;
        if (key === MOVE_KEY_DOWN) this.keys.down = true;
        if (key === MOVE_KEY_RIGHT) this.keys.right = true;

        // Update rotate key state
        if (key === ROTATE_KEY_LEFT) this.rotateKeys.left = true;
        if (key === ROTATE_KEY_RIGHT) this.rotateKeys.right = true;

        // Update zoom key state
        if (key === ZOOM_KEY_IN) this.zoomKeys.in = true;
        if (key === ZOOM_KEY_OUT) this.zoomKeys.out = true;

        this.emitMove();
        this.emitRotate();
        this.emitZoom();
    }

    handleKeyUp(ev: KeyboardEvent): void {
        const key = ev.key.toLowerCase();
        const isMoveKey = MOVE_KEYS.includes(key);
        const isRotateKey = ROTATE_KEYS.includes(key);
        const isZoomKey = ZOOM_KEYS.includes(key);

        if (!isMoveKey && !isRotateKey && !isZoomKey) {
            return;
        }

        ev.preventDefault();

        // Update move key state
        if (key === MOVE_KEY_SPRINT) this.keys.shift = false;
        if (key === MOVE_KEY_UP) this.keys.up = false;
        if (key === MOVE_KEY_LEFT) this.keys.left = false;
        if (key === MOVE_KEY_DOWN) this.keys.down = false;
        if (key === MOVE_KEY_RIGHT) this.keys.right = false;

        // Update rotate key state
        if (key === ROTATE_KEY_LEFT) this.rotateKeys.left = false;
        if (key === ROTATE_KEY_RIGHT) this.rotateKeys.right = false;

        // Update zoom key state
        if (key === ZOOM_KEY_IN) this.zoomKeys.in = false;
        if (key === ZOOM_KEY_OUT) this.zoomKeys.out = false;

        this.emitMove();
        this.emitRotate();
        this.emitZoom();
    }

    handleWheel(ev: WheelEvent): void {
        // Block wheel during interact (zoom key conflict resolution now in InputManager)
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
        this.handler.onZoomTo(pos, ev.deltaY);
    }

    reset(): void {
        // Emit END events for active gestures
        if (this.activeInteract && this.owningPointer) {
            this.handler.onInteractEnd(this.owningPointer.currentPos);
        } else if (this.activeDragPan && this.owningPointer) {
            this.handler.onDragPanEnd(this.owningPointer.currentPos);
        } else if (this.activeDragRotate && this.owningPointer) {
            this.handler.onDragRotateEnd(this.owningPointer.currentPos);
        }

        // Emit move event with zero direction if keys were pressed
        if (this.keys.up || this.keys.left || this.keys.down || this.keys.right) {
            this.keys = { up: false, left: false, down: false, right: false, shift: false };
            this.emitMove();
        }

        // Emit rotate event with zero direction if rotate keys were pressed
        if (this.rotateKeys.left || this.rotateKeys.right) {
            this.rotateKeys = { left: false, right: false };
            this.emitRotate();
        }

        // Emit zoom event with zero direction if zoom keys were pressed
        if (this.zoomKeys.in || this.zoomKeys.out) {
            this.zoomKeys = { in: false, out: false };
            this.emitZoom();
        }

        // Clear all state
        this.owningPointer = null;
        this.isOwningPointerDead = false;
        this.activeDragPan = false;
        this.activeDragRotate = false;
        this.activeInteract = false;
        this.prevDirection = { x: 0, y: 0 };
        this.prevShift = false;
        this.prevRotateDirection = 0;
        this.prevZoomDirection = 0;
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

    private calculateRotateDirection(): number {
        if (this.rotateKeys.left && !this.rotateKeys.right) return -1;
        if (this.rotateKeys.right && !this.rotateKeys.left) return 1;
        return 0;
    }

    private calculateZoomDirection(): number {
        if (this.zoomKeys.in && !this.zoomKeys.out) return -1;
        if (this.zoomKeys.out && !this.zoomKeys.in) return 1;
        return 0;
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

    private emitRotate(): void {
        const direction = this.calculateRotateDirection();

        if (direction !== this.prevRotateDirection) {
            this.prevRotateDirection = direction;
            this.handler.onRotate(direction);
        }
    }

    private emitZoom(): void {
        const direction = this.calculateZoomDirection();

        if (direction !== this.prevZoomDirection) {
            this.prevZoomDirection = direction;
            this.handler.onZoom(direction);
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
                !this.activeDragPan &&
                !this.activeInteract &&
                !this.activeDragRotate &&
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
        } else if (this.activeDragPan) {
            this.activeDragPan = false;
            this.handler.onDragPanEnd(releasedPos);
        } else if (this.activeDragRotate) {
            this.activeDragRotate = false;
            this.handler.onDragRotateEnd(releasedPos);
        }

        // Emit tap if qualified
        if (isTap) {
            this.handler.onTap(releasedPos);
        }
    }

    isActive(): boolean {
        return (
            this.owningPointer !== null ||
            this.keys.up ||
            this.keys.left ||
            this.keys.down ||
            this.keys.right ||
            this.rotateKeys.left ||
            this.rotateKeys.right ||
            this.zoomKeys.in ||
            this.zoomKeys.out
        );
    }
}
