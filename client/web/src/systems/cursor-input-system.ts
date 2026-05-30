import * as THREE from 'three';
import type { WorldCursor } from '../avatar/world-cursor';
import type { Camera } from '../engine/camera/camera';
import { EventDispatcher } from '../engine/events';
import type { InputHandler, Point, PointPair } from '../engine/input/input-handler';

export const INPUT_CONTROLLER_CHANGED = 'input:controller:changed';
export type InputControllerChangedEvent = { controller: 'touch' | 'desktop' };

export const CURSOR_INTERACT_START = 'cursor:interact_start';
export const CURSOR_INTERACT = 'cursor:interact';
export const CURSOR_INTERACT_END = 'cursor:interact_end';
export type CursorInteractEvent = { pos: THREE.Vector3 };

export class CursorInputSystem implements InputHandler {
    private readonly dispatcher: EventDispatcher;

    constructor(
        private readonly cursor: WorldCursor,
        private readonly camera: Camera,
        events: EventTarget
    ) {
        this.dispatcher = new EventDispatcher(events);
    }

    onSchemaChanged(schema: 'touch' | 'desktop'): void {
        this.dispatcher.dispatch<InputControllerChangedEvent>(INPUT_CONTROLLER_CHANGED, { controller: schema });
    }

    onMoveTo(pos: Point): void {
        const world = this.camera.screenToWorldPlanePoint(pos.x, pos.y);
        if (world) this.cursor.setPosition(world);
    }

    onRotateBy(angleDelta: number): void {
        this.cursor.rotateBy(angleDelta);
    }

    onZoomBy(delta: number): void {
        this.cursor.zoomBy(delta);
    }

    onMoveRate(x: number, y: number, sprint: boolean): void {
        this.cursor.moveRate.set(x, y);
        this.cursor.moveRateSprint = sprint;
    }

    onRotateRate(value: number): void {
        this.cursor.rotateRate = value;
    }

    onZoomRate(value: number): void {
        this.cursor.zoomRate = value;
    }

    onPinchStart(_start: PointPair): void {}
    onPinch(_start: PointPair, _prev: PointPair, _current: PointPair): void {}
    onPinchEnd(_start: PointPair, _end: PointPair): void {}

    onInteractStart(pos: Point): void {
        const world = this.camera.screenToWorldPlanePoint(pos.x, pos.y);
        if (world) this.dispatcher.dispatch<CursorInteractEvent>(CURSOR_INTERACT_START, { pos: world });
    }

    onInteract(_start: Point, _prev: Point, current: Point): void {
        const world = this.camera.screenToWorldPlanePoint(current.x, current.y);
        if (world) this.dispatcher.dispatch<CursorInteractEvent>(CURSOR_INTERACT, { pos: world });
    }

    onInteractEnd(_start: Point, end: Point): void {
        const world = this.camera.screenToWorldPlanePoint(end.x, end.y);
        if (world) this.dispatcher.dispatch<CursorInteractEvent>(CURSOR_INTERACT_END, { pos: world });
    }

    onGesture(_points: Float32Array): void {}
}
