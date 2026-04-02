// Input timing constants
export const TAP_THRESHOLD_MS = 500;
export const LONG_PRESS_MS = 500;
export const PINCH_TIMING_MS = 300;

// Input movement constants
export const MOVE_THRESHOLD_PX = 6;

// Input movement keys
export const MOVE_KEY_UP = 'w';
export const MOVE_KEY_LEFT = 'a';
export const MOVE_KEY_DOWN = 's';
export const MOVE_KEY_RIGHT = 'd';
export const MOVE_KEY_SPRINT = 'shift';
export const MOVE_KEYS: readonly string[] = [
    MOVE_KEY_UP,
    MOVE_KEY_LEFT,
    MOVE_KEY_DOWN,
    MOVE_KEY_RIGHT,
    MOVE_KEY_SPRINT
];

// Input sensitivity constants
export const ROTATE_SENSITIVITY = 0.005; // Radians per pixel
export const ZOOM_SENSITIVITY = 0.5; // Zoom speed multiplier for mouse wheel
export const ZOOM_DISTANCE_SCALE = 25; // Distance-dependent zoom scale factor

// World cursor movement
export const CURSOR_MOVE_SPEED = 1200; // units per second
export const CURSOR_SPRINT_MULTIPLIER = 3;

// RTS Camera parameters
export const MIN_CAMERA_DISTANCE = 40;
export const MAX_CAMERA_DISTANCE = 15000;
export const MIN_CAMERA_PITCH = 35 * (Math.PI / 180); // Radians
export const MAX_CAMERA_PITCH = 65 * (Math.PI / 180);

// Camera follow interpolation
export const CAMERA_BASE_LERP = 0.05;
export const CAMERA_LERP_DISTANCE_FACTOR = 0.1;
