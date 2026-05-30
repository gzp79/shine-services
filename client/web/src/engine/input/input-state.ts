import * as THREE from 'three';
import {
    CURSOR_MOVE_SPEED,
    CURSOR_SPRINT_MULTIPLIER,
    CURSOR_ROTATE_SPEED,
    CURSOR_ZOOM_SPEED
} from '../../constants';
import type { InputHandler, Point, PointPair } from './input-handler';

export type PendingInteractKind = 'start' | 'move' | 'end';

export interface PendingInteract {
    kind: PendingInteractKind;
    x: number;
    y: number;
}

export interface IInputState {
    readonly moveSpeed: Readonly<THREE.Vector2>;
    readonly rotateSpeed: number;
    readonly zoomSpeed: number;
    readonly pointerPos: Point | null;
    readonly pendingMoveTo: Point | null;
    readonly pendingRotateBy: number;
    readonly pendingZoomBy: number;
    readonly pendingInteracts: readonly PendingInteract[];
    readonly pendingInteractCount: number;
    readonly pendingSchemaChange: string | null;
}

export class InputState implements IInputState, InputHandler {
    readonly moveSpeed = new THREE.Vector2(0, 0);
    rotateSpeed = 0;
    zoomSpeed = 0;
    pointerPos: Point | null = null;
    pendingMoveTo: Point | null = null;
    pendingRotateBy = 0;
    pendingZoomBy = 0;
    readonly pendingInteracts: PendingInteract[] = [];
    pendingInteractCount = 0;
    pendingSchemaChange: string | null = null;

    clear(): void {
        this.pendingMoveTo = null;
        this.pendingRotateBy = 0;
        this.pendingZoomBy = 0;
        this.pendingInteractCount = 0;
        this.pendingSchemaChange = null;
    }

    onSchemaChanged(schema: string): void {
        this.pendingSchemaChange = schema;
    }

    onPointerAt(pos: Point): void {
        if (this.pointerPos) {
            this.pointerPos.x = pos.x;
            this.pointerPos.y = pos.y;
        } else {
            this.pointerPos = { x: pos.x, y: pos.y };
        }
    }

    onPointerLeave(): void {
        this.pointerPos = null;
    }

    onMoveTo(pos: Point): void {
        this.pendingMoveTo = pos;
    }

    onRotateBy(angleDelta: number): void {
        this.pendingRotateBy += angleDelta;
    }

    onZoomBy(delta: number): void {
        this.pendingZoomBy += delta;
    }

    onMoveRate(x: number, y: number, sprint: boolean): void {
        const len = Math.sqrt(x * x + y * y);
        if (len > 0) {
            const speed = CURSOR_MOVE_SPEED * (sprint ? CURSOR_SPRINT_MULTIPLIER : 1);
            this.moveSpeed.set((x / len) * speed, (y / len) * speed);
        } else {
            this.moveSpeed.set(0, 0);
        }
    }

    onRotateRate(value: number): void {
        this.rotateSpeed = -value * CURSOR_ROTATE_SPEED;
    }

    onZoomRate(value: number): void {
        this.zoomSpeed = value * CURSOR_ZOOM_SPEED;
    }

    onPinchStart(_start: PointPair): void {}
    onPinch(_start: PointPair, _prev: PointPair, _current: PointPair): void {}
    onPinchEnd(_start: PointPair, _end: PointPair): void {}

    onInteractStart(start: Point): void {
        this.pushInteract('start', start);
    }

    onInteract(_start: Point, _prev: Point, current: Point): void {
        this.pushInteract('move', current);
    }

    onInteractEnd(_start: Point, end: Point): void {
        this.pushInteract('end', end);
    }

    onGesture(_points: Float32Array): void {}

    private pushInteract(kind: PendingInteractKind, pos: Point): void {
        if (this.pendingInteractCount < this.pendingInteracts.length) {
            const slot = this.pendingInteracts[this.pendingInteractCount]!;
            slot.kind = kind;
            slot.x = pos.x;
            slot.y = pos.y;
        } else {
            this.pendingInteracts.push({ kind, x: pos.x, y: pos.y });
        }
        this.pendingInteractCount++;
    }
}
