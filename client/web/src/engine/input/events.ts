import type { Delta, Point } from './types';

// Controller selection
export const INPUT_CONTROLLER_CHANGED = 'input:controller:changed';
export type InputControllerChangedEvent = { controller: 'touch' | 'desktop' };

// Tap
export const INPUT_TAP = 'input:tap';
export type InputTapEvent = { pos: Point };

// Interact (long press + drag)
export const INPUT_INTERACT_START = 'input:interact:start';
export type InputInteractStartEvent = { pos: Point };

export const INPUT_INTERACT_DRAG = 'input:interact:drag';
export type InputInteractDragEvent = { start: Point; current: Point };

export const INPUT_INTERACT_END = 'input:interact:end';
export type InputInteractEndEvent = { pos: Point };

// Pan
export const INPUT_PAN_START = 'input:pan:start';
export type InputPanStartEvent = { pos: Point };

export const INPUT_PAN = 'input:pan';
export type InputPanEvent = { start: Point; current: Point };

export const INPUT_PAN_END = 'input:pan:end';
export type InputPanEndEvent = { pos: Point };

// Rotate (desktop only)
export const INPUT_ROTATE_START = 'input:rotate:start';
export type InputRotateStartEvent = { pos: Point };

export const INPUT_ROTATE = 'input:rotate';
export type InputRotateEvent = { start: Point; current: Point };

export const INPUT_ROTATE_END = 'input:rotate:end';
export type InputRotateEndEvent = { pos: Point };

// Pinch (touch only)
export const INPUT_PINCH_START = 'input:pinch:start';
export type InputPinchStartEvent = { start: [Point, Point]; current: [Point, Point] };

export const INPUT_PINCH = 'input:pinch';
export type InputPinchEvent = { start: [Point, Point]; current: [Point, Point] };

export const INPUT_PINCH_END = 'input:pinch:end';
export type InputPinchEndEvent = { start: [Point, Point]; current: [Point, Point] };

// Zoom (desktop only)
export const INPUT_ZOOM = 'input:zoom';
export type InputZoomEvent = { pos: Point; delta: number };

// WASD (desktop only)
export const INPUT_MOVE = 'input:wasd:move';
export type InputMoveEvent = { direction: Delta };
