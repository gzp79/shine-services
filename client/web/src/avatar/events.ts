import type * as THREE from 'three';

// Controller selection
export const INPUT_CONTROLLER_CHANGED = 'input:controller:changed';
export type InputControllerChangedEvent = { controller: 'touch' | 'desktop' };

export const CURSOR_MOVE_TO = 'cursor:move_to';
export type CursorMoveToEvent = { pos: THREE.Vector3 };

// Instant rotation offset in radians; applied immediately without integration
export const CURSOR_ROTATE_DELTA = 'cursor:rotate_delta';
export type CursorRotateDeltaEvent = { angleDelta: number };

// Instant zoom offset (world units); applied immediately without integration
export const CURSOR_ZOOM_DELTA = 'cursor:zoom_delta';
export type CursorZoomDeltaEvent = { delta: number };
