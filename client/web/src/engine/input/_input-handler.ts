// Screen position (top-left origin, y downward)
export type Point = { x: number; y: number };
// Screen relative (y downward)
export type Delta = { x: number; y: number };

/**
 * Callback interface for handling input events.
 * Implementations can dispatch events, map to world space, or apply effects directly.
 */
export interface InputHandler {
    onControllerChanged(controller: 'touch' | 'desktop'): void;
    onTap(pos: Point): void;
    onInteractStart(pos: Point): void;
    onInteractDrag(start: Point, current: Point): void;
    onInteractEnd(pos: Point): void;
    onPanStart(pos: Point): void;
    onPan(start: Point, current: Point): void;
    onPanEnd(pos: Point): void;
    onRotateStart(pos: Point): void;
    onRotate(start: Point, current: Point): void;
    onRotateEnd(pos: Point): void;
    onPinchStart(start: [Point, Point], current: [Point, Point]): void;
    onPinch(start: [Point, Point], current: [Point, Point]): void;
    onPinchEnd(start: [Point, Point], current: [Point, Point]): void;
    onZoom(pos: Point, delta: number): void;
    onMove(direction: Delta, isSprinting: boolean): void;
}
