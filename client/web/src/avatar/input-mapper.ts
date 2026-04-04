import * as THREE from 'three';
import { ROTATE_SENSITIVITY, ZOOM_SENSITIVITY } from '../constants';
import type { Camera } from '../engine/camera/camera';
import { EventDispatcher } from '../engine/events';
import type { Delta, InputHandler, Point } from '../engine/input';
import {
    CURSOR_MOVE,
    CURSOR_MOVE_TO,
    CURSOR_ROTATE,
    CURSOR_ZOOM,
    type CursorMoveEvent,
    type CursorMoveToEvent,
    type CursorRotateEvent,
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
    private rotateLastPos: Point | null = null;
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

    onInteractStart(_pos: Point): void {
        // Empty - no event emitted
    }

    onInteractDrag(_start: Point, _current: Point): void {
        // Empty - no event emitted
    }

    onInteractEnd(_pos: Point): void {
        // Empty - no event emitted
    }

    onPanStart(_pos: Point): void {
        // Empty - no event emitted
    }

    onPan(_start: Point, current: Point): void {
        this.emitMoveTo(current);
    }

    onPanEnd(current: Point): void {
        this.emitMoveTo(current);
    }

    onRotateStart(pos: Point): void {
        this.rotateLastPos = pos;
    }

    onRotate(_start: Point, current: Point): void {
        if (!this.rotateLastPos) return;

        // Calculate rotation from horizontal screen delta
        const deltaX = current.x - this.rotateLastPos.x;
        const angleDelta = deltaX * ROTATE_SENSITIVITY;

        this.rotateLastPos = current;

        // Emit rotation delta (WorldCursor will update its yaw)
        this.dispatcher.dispatch<CursorRotateEvent>(CURSOR_ROTATE, { angleDelta });

        // If WASD is active, update velocity to match new cursor orientation
        if (this.currentMoveInput.x !== 0 || this.currentMoveInput.y !== 0) {
            this.emitMove();
        }
    }

    onRotateEnd(_pos: Point): void {
        this.rotateLastPos = null;
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

        // Calculate zoom delta (negative = zoom out, positive = zoom in)
        const delta = this.pinchStartDistance - currentDistance;
        this.pinchStartDistance = currentDistance;

        this.dispatcher.dispatch<CursorZoomEvent>(CURSOR_ZOOM, { delta });
    }

    onPinchEnd(_start: [Point, Point], _current: [Point, Point]): void {
        this.pinchStartDistance = null;
    }

    onZoom(_pos: Point, delta: number): void {
        // Mouse wheel: positive delta = zoom in (decrease distance), negative = zoom out (increase distance)
        this.dispatcher.dispatch<CursorZoomEvent>(CURSOR_ZOOM, { delta: delta * ZOOM_SENSITIVITY });
    }

    onMove(direction: Delta, isSprinting: boolean): void {
        // Store the input-space direction and sprint state
        this.currentMoveInput = direction;
        this.currentSprinting = isSprinting;

        // Transform input direction to world space and emit
        this.emitMove();
    }

    private emitMove(): void {
        // Transform input direction to world space using cursor orientation (not blended camera)
        if (this.currentMoveInput.x === 0 && this.currentMoveInput.y === 0) {
            // No movement - emit zero vector
            this.dispatcher.dispatch<CursorMoveEvent>(CURSOR_MOVE, {
                direction: new THREE.Vector3(0, 0, 0),
                isSprinting: false
            });
            return;
        }

        // Get cursor orientation from WorldCursor (instant rotation, not blended like camera)
        const cursorYaw = this.worldCursor.getCameraTarget().yaw;

        // Calculate forward and right vectors based on cursor yaw
        const forward = new THREE.Vector3(Math.sin(cursorYaw), Math.cos(cursorYaw), 0);
        const right = new THREE.Vector3(Math.cos(cursorYaw), -Math.sin(cursorYaw), 0);

        // Combine input axes
        forward.multiplyScalar(-this.currentMoveInput.y); // -y because up key is negative y in input
        right.multiplyScalar(this.currentMoveInput.x);
        forward.add(right);
        forward.normalize();

        this.dispatcher.dispatch<CursorMoveEvent>(CURSOR_MOVE, {
            direction: forward,
            isSprinting: this.currentSprinting
        });
    }

    private emitMoveTo(screenPos: Point): void {
        // Convert screen position to normalized device coordinates (-1 to 1)
        const ndcX = (screenPos.x / window.innerWidth) * 2 - 1;
        const ndcY = -(screenPos.y / window.innerHeight) * 2 + 1; // Invert Y

        // Create a ray from the camera through the screen point
        const raycaster = new THREE.Raycaster();
        raycaster.setFromCamera(new THREE.Vector2(ndcX, ndcY), this.camera.camera);

        // Intersect with a plane at y=0 (ground plane)
        const plane = new THREE.Plane(new THREE.Vector3(0, 0, 1), 0);
        const intersectionPoint = new THREE.Vector3();
        raycaster.ray.intersectPlane(plane, intersectionPoint);

        if (intersectionPoint) {
            intersectionPoint.z = 0;
            this.dispatcher.dispatch<CursorMoveToEvent>(CURSOR_MOVE_TO, {
                pos: intersectionPoint
            });
        }
    }
}
