import * as THREE from 'three';
import { ROTATE_KEY_SPEED, ROTATE_SENSITIVITY, ZOOM_KEY_SPEED, ZOOM_SENSITIVITY } from '../constants';
import type { Camera } from '../engine/camera/camera';
import { EventDispatcher } from '../engine/events';
import type { Delta, InputHandler, Point } from '../engine/input';
import {
    CURSOR_MOVE,
    CURSOR_MOVE_TO,
    CURSOR_ROTATE,
    CURSOR_ROTATE_DELTA,
    CURSOR_ZOOM,
    CURSOR_ZOOM_DELTA,
    type CursorMoveEvent,
    type CursorMoveToEvent,
    type CursorRotateDeltaEvent,
    type CursorRotateEvent,
    type CursorZoomDeltaEvent,
    type CursorZoomEvent,
    INPUT_CONTROLLER_CHANGED,
    type InputControllerChangedEvent
} from './events';
import type { WorldCursor } from './world-cursor';

/**
 * Maps low-level screen-space input events to cursor-aware events.
 * Uses Camera for raycasting and WorldCursor for orientation.
 */
export class InputMapper implements InputHandler {
    private dispatcher: EventDispatcher;
    private dragRotateLastPos: Point | null = null;
    private pinchStartDistance: number | null = null;
    private currentMoveInput: Delta = { x: 0, y: 0 };
    private currentSprinting = false;

    constructor(
        private readonly camera: Camera,
        private readonly worldCursor: WorldCursor,
        events: EventTarget
    ) {
        this.dispatcher = new EventDispatcher(events);
    }

    onControllerChanged(controller: 'touch' | 'desktop'): void {
        this.dispatcher.dispatch<InputControllerChangedEvent>(INPUT_CONTROLLER_CHANGED, { controller });
    }

    onTap(pos: Point): void {
        this.emitMoveTo(pos);
    }

    onInteractStart(_pos: Point): void {}
    onInteractDrag(_start: Point, _current: Point): void {}
    onInteractEnd(_pos: Point): void {}

    onDragPanStart(_pos: Point): void {}

    onDragPan(_start: Point, current: Point): void {
        this.emitMoveTo(current);
    }

    onDragPanEnd(current: Point): void {
        this.emitMoveTo(current);
    }

    onDragRotateStart(pos: Point): void {
        this.dragRotateLastPos = pos;
    }

    onDragRotate(_start: Point, current: Point): void {
        if (!this.dragRotateLastPos) return;

        const deltaX = current.x - this.dragRotateLastPos.x;
        const angleDelta = deltaX * ROTATE_SENSITIVITY;
        this.dragRotateLastPos = current;

        this.dispatcher.dispatch<CursorRotateDeltaEvent>(CURSOR_ROTATE_DELTA, { angleDelta });

        // If WASD is active, update velocity to match new cursor orientation
        if (this.currentMoveInput.x !== 0 || this.currentMoveInput.y !== 0) {
            this.emitMove();
        }
    }

    onDragRotateEnd(_pos: Point): void {
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

        this.dispatcher.dispatch<CursorZoomEvent>(CURSOR_ZOOM, { delta });
    }

    onPinchEnd(_start: [Point, Point], _current: [Point, Point]): void {
        this.pinchStartDistance = null;
    }

    onZoomTo(_pos: Point, delta: number): void {
        this.dispatcher.dispatch<CursorZoomDeltaEvent>(CURSOR_ZOOM_DELTA, { delta: delta * ZOOM_SENSITIVITY });
    }

    onZoom(direction: number): void {
        this.dispatcher.dispatch<CursorZoomEvent>(CURSOR_ZOOM, { direction: direction * ZOOM_KEY_SPEED });
    }

    onMove(direction: Delta, isSprinting: boolean): void {
        this.currentMoveInput = direction;
        this.currentSprinting = isSprinting;
        this.emitMove();
    }

    onRotate(direction: number): void {
        // Emit rate scaled by speed; WorldCursor integrates direction * deltaTime each frame
        this.dispatcher.dispatch<CursorRotateEvent>(CURSOR_ROTATE, { direction: direction * ROTATE_KEY_SPEED });
    }

    private emitMove(): void {
        if (this.currentMoveInput.x === 0 && this.currentMoveInput.y === 0) {
            this.dispatcher.dispatch<CursorMoveEvent>(CURSOR_MOVE, {
                direction: new THREE.Vector3(0, 0, 0),
                isSprinting: false
            });
            return;
        }

        const cursorYaw = this.worldCursor.getCameraTarget().yaw;

        const forward = new THREE.Vector3(Math.sin(cursorYaw), Math.cos(cursorYaw), 0);
        const right = new THREE.Vector3(Math.cos(cursorYaw), -Math.sin(cursorYaw), 0);

        forward.multiplyScalar(-this.currentMoveInput.y);
        right.multiplyScalar(this.currentMoveInput.x);
        forward.add(right);
        forward.normalize();

        this.dispatcher.dispatch<CursorMoveEvent>(CURSOR_MOVE, {
            direction: forward,
            isSprinting: this.currentSprinting
        });
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
