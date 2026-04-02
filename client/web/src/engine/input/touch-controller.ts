import { EventDispatcher } from '../events';
import { LONG_PRESS_MS, MOVE_THRESHOLD_PX, PINCH_TIMING_MS, TAP_THRESHOLD_MS } from './constants';
import {
    INPUT_INTERACT_DRAG,
    INPUT_INTERACT_END,
    INPUT_INTERACT_START,
    INPUT_PAN,
    INPUT_PAN_END,
    INPUT_PAN_START,
    INPUT_PINCH,
    INPUT_PINCH_END,
    INPUT_PINCH_START,
    INPUT_TAP,
    type InputInteractDragEvent,
    type InputInteractEndEvent,
    type InputInteractStartEvent,
    type InputPanEndEvent,
    type InputPanEvent,
    type InputPanStartEvent,
    type InputPinchEndEvent,
    type InputPinchEvent,
    type InputPinchStartEvent,
    type InputTapEvent
} from './events';
import type { Point, PointerInfo } from './types';

export class TouchController {
    private pointers = new Map<number, PointerInfo>();
    private inactivePointers = new Set<number>(); // Post-pinch remaining fingers
    private activePan = false;
    private activePinch = false;
    private activeInteract = false;
    private longPressTimer: number | null = null;
    private pinchStart: [Point, Point] | null = null;
    private dispatcher: EventDispatcher;

    constructor(dispatcher: EventDispatcher) {
        this.dispatcher = dispatcher;
    }

    handlePointerDown(ev: PointerEvent): void {
        const p = this.pos(ev);
        const pointerInfo: PointerInfo = {
            id: ev.pointerId,
            startPos: p,
            currentPos: p,
            downTime: performance.now(),
            button: ev.button
        };
        this.pointers.set(ev.pointerId, pointerInfo);

        if (this.pointers.size === 1) {
            // Single pointer - start long press timer for interact
            this.activePan = false;
            this.activeInteract = false;
            this.cancelLongPressTimer();

            this.longPressTimer = window.setTimeout(() => {
                if (this.pointers.size === 1 && !this.activePan) {
                    this.activeInteract = true;
                    this.dispatcher.dispatch<InputInteractStartEvent>(INPUT_INTERACT_START, { pos: p });
                }
            }, LONG_PRESS_MS);
        } else if (this.pointers.size === 2) {
            // Second pointer - check for pinch
            this.cancelLongPressTimer();

            const pointerIds = [...this.pointers.keys()];
            const firstId = pointerIds[0];
            const firstPointer = this.pointers.get(firstId)!;
            const firstMoved = this.dist(firstPointer.startPos, firstPointer.currentPos);

            // Check timing between first and second pointer
            const timeSinceFirst = performance.now() - firstPointer.downTime;

            if (firstMoved < MOVE_THRESHOLD_PX && timeSinceFirst < PINCH_TIMING_MS) {
                // Start pinch
                const pts = [...this.pointers.values()];
                const start: [Point, Point] = [pts[0].currentPos, pts[1].currentPos];
                this.activePinch = true;
                this.activePan = false;
                this.pinchStart = start;
                this.dispatcher.dispatch<InputPinchStartEvent>(INPUT_PINCH_START, {
                    start,
                    current: start
                });
            } else {
                // Pan already started or too slow - ignore second pointer
                this.pointers.delete(ev.pointerId);
            }
        } else if (this.pointers.size > 2) {
            // Third+ pointer - ignore
            this.pointers.delete(ev.pointerId);
        }
    }

    handlePointerMove(ev: PointerEvent): void {
        const pointer = this.pointers.get(ev.pointerId);
        if (!pointer) return;

        // Inactive pointers (post-pinch remaining fingers) cannot interact
        if (this.inactivePointers.has(ev.pointerId)) {
            return;
        }

        const newPos = this.pos(ev);
        pointer.currentPos = newPos;

        // Two-finger pinch
        if (this.activePinch && this.pointers.size === 2 && this.pinchStart) {
            const pts = [...this.pointers.values()];
            const current: [Point, Point] = [pts[0].currentPos, pts[1].currentPos];
            this.dispatcher.dispatch<InputPinchEvent>(INPUT_PINCH, {
                start: this.pinchStart,
                current
            });
            return;
        }

        // Single pointer movement
        if (this.pointers.size === 1) {
            const totalMoved = this.dist(pointer.startPos, newPos);

            // Cancel long press timer if moved beyond threshold
            if (totalMoved > MOVE_THRESHOLD_PX) {
                this.cancelLongPressTimer();
            }

            // Interact drag (after long press)
            if (this.activeInteract) {
                this.dispatcher.dispatch<InputInteractDragEvent>(INPUT_INTERACT_DRAG, {
                    start: pointer.startPos,
                    current: newPos
                });
                return;
            }

            // Pan (normal drag)
            if (!this.activePan && totalMoved > MOVE_THRESHOLD_PX) {
                this.activePan = true;
                this.dispatcher.dispatch<InputPanStartEvent>(INPUT_PAN_START, { pos: pointer.startPos });
            }

            if (this.activePan) {
                this.dispatcher.dispatch<InputPanEvent>(INPUT_PAN, {
                    start: pointer.startPos,
                    current: newPos
                });
            }
        }
    }

    handlePointerUp(ev: PointerEvent): void {
        const pointer = this.pointers.get(ev.pointerId);
        if (!pointer) return;

        // Handle inactive pointers (post-pinch remaining fingers)
        if (this.inactivePointers.has(ev.pointerId)) {
            this.inactivePointers.delete(ev.pointerId);
            this.pointers.delete(ev.pointerId);
            return;
        }

        const duration = performance.now() - pointer.downTime;

        // Capture pinch end data before removing pointer
        let shouldEmitPinchEnd = false;
        let pinchEndData: InputPinchEndEvent | null = null;

        if (this.activePinch && this.pointers.size === 2 && this.pinchStart) {
            const pts = [...this.pointers.values()];
            const current: [Point, Point] = [pts[0].currentPos, pts[1].currentPos];
            pinchEndData = { start: this.pinchStart, current };
            shouldEmitPinchEnd = true;
        }

        // Remove pointer
        this.pointers.delete(ev.pointerId);

        // Cancel long press timer
        this.cancelLongPressTimer();

        // Handle pinch end
        if (shouldEmitPinchEnd && pinchEndData) {
            this.activePinch = false;
            this.pinchStart = null;
            this.dispatcher.dispatch<InputPinchEndEvent>(INPUT_PINCH_END, pinchEndData);

            // Remaining pointer becomes inactive (cannot start new gesture until lifted)
            if (this.pointers.size === 1) {
                const remainingId = [...this.pointers.keys()][0];
                this.inactivePointers.add(remainingId);
                this.activePan = false;
            }
            return;
        }

        // Handle interact end
        if (this.activeInteract && this.pointers.size === 0) {
            this.activeInteract = false;
            this.dispatcher.dispatch<InputInteractEndEvent>(INPUT_INTERACT_END, { pos: pointer.currentPos });
            return;
        }

        // Handle pan end
        if (this.activePan && this.pointers.size === 0) {
            this.activePan = false;
            this.dispatcher.dispatch<InputPanEndEvent>(INPUT_PAN_END, { pos: pointer.currentPos });
            return;
        }

        // Tap (quick press, no movement)
        const totalMoved = this.dist(pointer.startPos, pointer.currentPos);
        if (!this.activePan && !this.activeInteract && duration < TAP_THRESHOLD_MS && totalMoved <= MOVE_THRESHOLD_PX) {
            this.dispatcher.dispatch<InputTapEvent>(INPUT_TAP, { pos: pointer.currentPos });
        }

        // Reset states
        if (this.pointers.size === 0) {
            this.activePan = false;
            this.activeInteract = false;
            this.activePinch = false;
        }
    }

    handlePointerCancel(ev: PointerEvent): void {
        const pointer = this.pointers.get(ev.pointerId);
        if (!pointer) return;

        // Handle inactive pointers (post-pinch remaining fingers)
        if (this.inactivePointers.has(ev.pointerId)) {
            this.inactivePointers.delete(ev.pointerId);
            this.pointers.delete(ev.pointerId);
            return;
        }

        // Emit END event for this pointer's gesture
        let justEndedPinch = false;
        if (this.activeInteract) {
            this.dispatcher.dispatch<InputInteractEndEvent>(INPUT_INTERACT_END, {
                pos: pointer.currentPos
            });
            this.activeInteract = false;
        } else if (this.activePan) {
            this.dispatcher.dispatch<InputPanEndEvent>(INPUT_PAN_END, { pos: pointer.currentPos });
            this.activePan = false;
        } else if (this.activePinch && this.pinchStart) {
            const pts = [...this.pointers.values()];
            if (pts.length >= 2) {
                const current: [Point, Point] = [pts[0].currentPos, pts[1].currentPos];
                this.dispatcher.dispatch<InputPinchEndEvent>(INPUT_PINCH_END, {
                    start: this.pinchStart,
                    current
                });
            }
            this.activePinch = false;
            this.pinchStart = null;
            justEndedPinch = true;
        }

        // Remove pointer and cancel timer
        this.pointers.delete(ev.pointerId);
        this.cancelLongPressTimer();

        // Mark remaining pointer as inactive if we just ended a pinch
        if (justEndedPinch && this.pointers.size === 1) {
            const remainingId = [...this.pointers.keys()][0];
            this.inactivePointers.add(remainingId);
        }

        // Reset if no pointers left
        if (this.pointers.size === 0) {
            this.activePan = false;
            this.activePinch = false;
            this.activeInteract = false;
        }
    }

    reset(): void {
        // Emit END events for active gestures
        if (this.activeInteract) {
            const pointer = [...this.pointers.values()][0];
            if (pointer) {
                this.dispatcher.dispatch<InputInteractEndEvent>(INPUT_INTERACT_END, {
                    pos: pointer.currentPos
                });
            }
        } else if (this.activePan) {
            const pointer = [...this.pointers.values()][0];
            if (pointer) {
                this.dispatcher.dispatch<InputPanEndEvent>(INPUT_PAN_END, { pos: pointer.currentPos });
            }
        } else if (this.activePinch && this.pinchStart) {
            const pts = [...this.pointers.values()];
            if (pts.length >= 2) {
                const current: [Point, Point] = [pts[0].currentPos, pts[1].currentPos];
                this.dispatcher.dispatch<InputPinchEndEvent>(INPUT_PINCH_END, {
                    start: this.pinchStart,
                    current
                });
            }
        }

        // Clear all state
        this.pointers.clear();
        this.inactivePointers.clear();
        this.activePan = false;
        this.activePinch = false;
        this.activeInteract = false;
        this.pinchStart = null;
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

    isActive(): boolean {
        return this.pointers.size > 0;
    }
}
