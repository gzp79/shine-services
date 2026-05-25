import type * as THREE from 'three';

// Controller selection
export const INPUT_CONTROLLER_CHANGED = 'input:controller:changed';
export type InputControllerChangedEvent = { controller: 'touch' | 'desktop' };

// Continuous movement rate (world units/s); WorldCursor integrates per frame until direction resets to zero
export const CURSOR_MOVE = 'cursor:move';
export type CursorMoveEvent = { direction: THREE.Vector3; isSprinting: boolean };

export const CURSOR_MOVE_TO = 'cursor:move_to';
export type CursorMoveToEvent = { pos: THREE.Vector3 };

// Continuous rotation rate (rad/s); WorldCursor integrates per frame until direction resets to 0
export const CURSOR_ROTATE = 'cursor:rotate';
export type CursorRotateEvent = { direction: number };

// Instant rotation offset in radians; applied immediately without integration
export const CURSOR_ROTATE_DELTA = 'cursor:rotate_delta';
export type CursorRotateDeltaEvent = { angleDelta: number };

export const CURSOR_ZOOM = 'cursor:zoom';
export type CursorZoomEvent = { delta: number };
