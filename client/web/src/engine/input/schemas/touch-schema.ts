import type { Point } from '../input-handler';
import type { InputSchema } from './input-schema';
import { RawSingleTouch } from '../raw/raw-single-touch';
import { RawTwoFingerGesture } from '../raw/raw-two-finger-gesture';

/**
 * TouchSchema handles touch input (single and two-finger gestures).
 * Built from generic raw inputs (RawSingleTouch, RawTwoFingerGesture).
 *
 * Conflict rules (first to start wins):
 * - Single-finger ↔ two-finger gestures
 */
export class TouchSchema implements InputSchema {
    onMoveTo?: (screenPos: Point) => void;
    onRotateBy?: (angleDelta: number) => void;
    onZoomBy?: (delta: number) => void;
    onMoveRate?: (x: number, y: number, sprint: boolean) => void;
    onRotateRate?: (value: number) => void;
    onZoomRate?: (value: number) => void;
    onPinchStart?: (pos1: Point, pos2: Point) => void;
    onPinch?: (pos1: Point, pos2: Point) => void;
    onPinchEnd?: () => void;
    onInteractStart?: (pos: Point) => void;
    onInteract?: (pos: Point) => void;
    onInteractEnd?: (pos: Point) => void;

    private readonly singleTouch: RawSingleTouch;
    private readonly twoFingerGesture: RawTwoFingerGesture;

    constructor(container: HTMLElement) {

        this.singleTouch = new RawSingleTouch(container);
        this.twoFingerGesture = new RawTwoFingerGesture(container);

        // Wire single-finger gestures
        this.singleTouch.onTap = (pos) => this.onMoveTo?.(pos);
        this.singleTouch.onDragStart = (pos) => {
            this.twoFingerGesture.enabled = false;
            this.onMoveTo?.(pos);
        };
        this.singleTouch.onDrag = (pos) => {
            this.onMoveTo?.(pos);
        };
        this.singleTouch.onDragEnd = (pos) => {
            this.onMoveTo?.(pos);
            this.twoFingerGesture.enabled = true;
        };
        this.singleTouch.onLongDragStart = (pos) => {
            this.twoFingerGesture.enabled = false;
            this.onInteractStart?.(pos);
        };
        this.singleTouch.onLongDrag = (pos) => {
            this.onInteract?.(pos);
        };
        this.singleTouch.onLongDragEnd = (pos) => {
            this.onInteractEnd?.(pos);
            this.twoFingerGesture.enabled = true;
        };

        // Wire two-finger gestures
        this.twoFingerGesture.onStart = (pos1, pos2) => {
            this.singleTouch.enabled = false;
            this.onPinchStart?.(pos1, pos2);
        };
        this.twoFingerGesture.onPinch = (pos1, pos2) => {
            this.onPinch?.(pos1, pos2);
        };
        this.twoFingerGesture.onEnd = () => {
            this.onPinchEnd?.();
        };
        this.twoFingerGesture.onAllFingersReleased = () => {
            this.singleTouch.enabled = true;
        };
    }

    isActive(): boolean {
        return this.singleTouch.isActive() || this.twoFingerGesture.isActive();
    }

    dispose(): void {
        this.singleTouch.dispose();
        this.twoFingerGesture.dispose();
    }
}
