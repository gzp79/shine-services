import type * as THREE from 'three';

// Controller selection
export const INPUT_CONTROLLER_CHANGED = 'input:controller:changed';
export type InputControllerChangedEvent = { controller: 'touch' | 'desktop' };

// World cursor control events
export const CURSOR_MOVE = 'cursor:move';
export type CursorMoveEvent = { direction: THREE.Vector3; isSprinting: boolean };

export const CURSOR_MOVE_TO = 'cursor:move_to';
export type CursorMoveToEvent = { pos: THREE.Vector3 };

export const CURSOR_ROTATE = 'cursor:rotate';
export type CursorRotateEvent = { angleDelta: number };

export const CURSOR_ZOOM = 'cursor:zoom';
export type CursorZoomEvent = { delta: number };
