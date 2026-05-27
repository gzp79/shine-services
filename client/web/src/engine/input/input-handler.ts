export type Point = { x: number; y: number };

export interface InputHandler {
    onSchemaChanged(schema: 'touch' | 'desktop'): void;

    onMoveTo(pos: Point): void;
    onRotateBy(angleDelta: number): void;
    onZoomBy(delta: number): void;

    onMoveRate(x: number, y: number, sprint: boolean): void;
    onRotateRate(value: number): void;
    onZoomRate(value: number): void;

    onPinchStart(pos1: Point, pos2: Point): void;
    onPinch(pos1: Point, pos2: Point): void;
    onPinchEnd(): void;
    onInteractStart(pos: Point): void;
    onInteract(pos: Point): void;
    onInteractEnd(pos: Point): void;
}
