import type { Camera } from '../engine/camera/camera';
import type { Delta, InputHandler, Point } from '../engine/input/_input-handler';
import type { WorldCursor } from './world-cursor';
import { ROTATE_KEY_SPEED, ROTATE_SENSITIVITY, ZOOM_KEY_SPEED, ZOOM_SENSITIVITY } from '../constants';
import { EventDispatcher } from '../engine/events';
import {
    CURSOR_MOVE_TO,
    CURSOR_ROTATE_DELTA,
    CURSOR_ZOOM_DELTA,
    type CursorMoveToEvent,
    type CursorRotateDeltaEvent,
    type CursorZoomDeltaEvent
} from './events';

export interface LocomotionState {
    move: Delta;
    rotateRate: number;
    zoomRate: number;
    sprint: boolean;
}

/**
 * InputManager centralizes locomotion state and conflict resolution.
 * Implements InputHandler to receive input events from InputController,
 * stores locomotion state for polling by systems.
 */
export class InputManager implements InputHandler {
    private locomotionState: LocomotionState;
    private activeDragPan = false;
    private activeDragRotate = false;
    private dragRotateLastPos: Point | null = null;
    private pinchStartDistance: number | null = null;
    private readonly dispatcher: EventDispatcher;

    constructor(
        private readonly camera: Camera,
        private readonly worldCursor: WorldCursor,
        private readonly events: EventTarget
    ) {
        this.locomotionState = {
            move: { x: 0, y: 0 },
            rotateRate: 0,
            zoomRate: 0,
            sprint: false
        };
        this.dispatcher = new EventDispatcher(events);
    }

    getLocomotionState(): Readonly<LocomotionState> {
        return this.locomotionState;
    }

    private get isWASDActive(): boolean {
        return this.locomotionState.move.x !== 0 || this.locomotionState.move.y !== 0;
    }

    private get isQEActive(): boolean {
        return this.locomotionState.rotateRate !== 0;
    }

    private get isRFActive(): boolean {
        return this.locomotionState.zoomRate !== 0;
    }

    onControllerChanged(_controller: 'touch' | 'desktop'): void {
        // No-op for now
    }

    onTap(pos: Point): void {
        this.emitMoveTo(pos);
    }

    onInteractStart(_pos: Point): void {
        // No-op for now
    }

    onInteractDrag(_start: Point, _current: Point): void {
        // No-op for now
    }

    onInteractEnd(_pos: Point): void {
        // No-op for now
    }

    onDragPanStart(_pos: Point): void {
        // Block if WASD active
        if (this.isWASDActive) {
            return;
        }
        this.activeDragPan = true;
    }

    onDragPan(_start: Point, current: Point): void {
        // Stop drag-pan if WASD becomes active
        if (this.isWASDActive) {
            this.activeDragPan = false;
            return;
        }
        if (!this.activeDragPan) return;

        this.emitMoveTo(current);
    }

    onDragPanEnd(current: Point): void {
        // Emit final moveTo if drag-pan was active
        if (this.activeDragPan) {
            this.emitMoveTo(current);
        }
        this.activeDragPan = false;
    }

    onDragRotateStart(pos: Point): void {
        // Block if Q/E active
        if (this.isQEActive) {
            return;
        }
        this.activeDragRotate = true;
        this.dragRotateLastPos = pos;
    }

    onDragRotate(_start: Point, current: Point): void {
        // Stop drag-rotate if Q/E becomes active
        if (this.isQEActive) {
            this.activeDragRotate = false;
            this.dragRotateLastPos = null;
            return;
        }
        if (!this.activeDragRotate || !this.dragRotateLastPos) return;

        const deltaX = current.x - this.dragRotateLastPos.x;
        const angleDelta = deltaX * ROTATE_SENSITIVITY;
        this.dragRotateLastPos = current;

        this.dispatcher.dispatch<CursorRotateDeltaEvent>(CURSOR_ROTATE_DELTA, { angleDelta });
    }

    onDragRotateEnd(_pos: Point): void {
        this.activeDragRotate = false;
        this.dragRotateLastPos = null;
    }

    onPinchStart(_start: [Point, Point], current: [Point, Point]): void {
        const dx = current[1].x - current[0].x;
        const dy = current[1].y - current[0].y;
        this.pinchStartDistance = Math.sqrt(dx * dx + dy * dy);
    }

    onPinch(_start: [Point, Point], current: [Point, Point]): void {
        if (this.pinchStartDistance === null) return;

        const dx = current[1].x - current[0].x;
        const dy = current[1].y - current[0].y;
        const currentDistance = Math.sqrt(dx * dx + dy * dy);

        const delta = this.pinchStartDistance - currentDistance;
        this.pinchStartDistance = currentDistance;

        this.dispatcher.dispatch<CursorZoomDeltaEvent>(CURSOR_ZOOM_DELTA, { delta });
    }

    onPinchEnd(_start: [Point, Point], _current: [Point, Point]): void {
        this.pinchStartDistance = null;
    }

    onZoomTo(_pos: Point, delta: number): void {
        // Block if R/F active
        if (this.isRFActive) {
            return;
        }
        this.dispatcher.dispatch<CursorZoomDeltaEvent>(CURSOR_ZOOM_DELTA, { delta: delta * ZOOM_SENSITIVITY });
    }

    onMove(direction: Delta, isSprinting: boolean): void {
        // Normalize if direction is non-zero
        const lengthSquared = direction.x * direction.x + direction.y * direction.y;
        if (lengthSquared > 0) {
            const length = Math.sqrt(lengthSquared);
            this.locomotionState.move = {
                x: direction.x / length,
                y: direction.y / length
            };
        } else {
            this.locomotionState.move = { x: 0, y: 0 };
        }
        this.locomotionState.sprint = isSprinting;
    }

    onRotate(direction: number): void {
        // Block if drag-rotate active (bidirectional)
        if (this.activeDragRotate) {
            return;
        }
        this.locomotionState.rotateRate = direction * ROTATE_KEY_SPEED;
    }

    onZoom(direction: number): void {
        this.locomotionState.zoomRate = direction * ZOOM_KEY_SPEED;
    }

    private emitMoveTo(screenPos: Point): void {
        const intersectionPoint = this.camera.screenToWorldPlanePoint(screenPos.x, screenPos.y);
        if (intersectionPoint) {
            intersectionPoint.z = 0;
            this.dispatcher.dispatch<CursorMoveToEvent>(CURSOR_MOVE_TO, {
                pos: intersectionPoint
            });
        }
    }
}
