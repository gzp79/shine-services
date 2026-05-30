export type Point = { x: number; y: number };
export type PointPair = [Point,Point];

export interface InputHandler {
    onSchemaChanged(schema: string): void;

    onMoveTo(pos: Point): void;
    onRotateBy(angleDelta: number): void;
    onZoomBy(delta: number): void;

    onMoveRate(x: number, y: number, sprint: boolean): void;
    onRotateRate(value: number): void;
    onZoomRate(value: number): void;
    
    onPinchStart(pos: PointPair): void;
    onPinch(start: PointPair, prev: PointPair, current: PointPair): void;    
    onPinchEnd(start: PointPair, end: PointPair): void;

    onInteractStart(start: Point): void;
    onInteract(start: Point, prev: Point, current: Point): void;
    onInteractEnd(start: Point, end: Point): void;
}
