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
    onDragPanStart(pos: Point): void;
    onDragPan(start: Point, current: Point): void;
    onDragPanEnd(pos: Point): void;
    onDragRotateStart(pos: Point): void;
    onDragRotate(start: Point, current: Point): void;
    onDragRotateEnd(pos: Point): void;
    onPinchStart(start: [Point, Point], current: [Point, Point]): void;
    onPinch(start: [Point, Point], current: [Point, Point]): void;
    onPinchEnd(start: [Point, Point], current: [Point, Point]): void;
    onZoomTo(pos: Point, delta: number): void;
    onMove(direction: Delta, isSprinting: boolean): void;
    onRotate(direction: number): void;
    onZoom(direction: number): void;
}
