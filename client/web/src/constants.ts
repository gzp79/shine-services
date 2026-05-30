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

// Input rotate keys
export const ROTATE_KEY_LEFT = 'q';
export const ROTATE_KEY_RIGHT = 'e';
export const ROTATE_KEY_SPEED = 90 * (Math.PI / 180); // Radians per second
export const ROTATE_KEYS: readonly string[] = [ROTATE_KEY_LEFT, ROTATE_KEY_RIGHT];

// Input zoom keys
export const ZOOM_KEY_IN = 'r';
export const ZOOM_KEY_OUT = 'f';
export const ZOOM_KEY_SPEED = 250; // Zoom units per second (distance-scaled in consumer)
export const ZOOM_KEYS: readonly string[] = [ZOOM_KEY_IN, ZOOM_KEY_OUT];

// Input sensitivity constants
export const ROTATE_SENSITIVITY = 0.005; // Radians per pixel
export const ZOOM_SENSITIVITY = 0.5; // Zoom speed multiplier for mouse wheel
export const ZOOM_DISTANCE_SCALE = 25; // Distance-dependent zoom scale factor

// World cursor movement
export const CURSOR_MOVE_SPEED = 1200; // units per second
export const CURSOR_SPRINT_MULTIPLIER = 3;
export const CURSOR_ROTATE_SPEED = Math.PI; // radians/second
export const CURSOR_ZOOM_SPEED = 10; // units/second

// RTS Camera parameters
export const MIN_CAMERA_DISTANCE = 40;
export const MAX_CAMERA_DISTANCE = 15000;
export const MIN_CAMERA_PITCH = 35 * (Math.PI / 180); // Radians
export const MAX_CAMERA_PITCH = 65 * (Math.PI / 180);

// Camera follow interpolation
export const CAMERA_BASE_LERP = 0.05;
export const CAMERA_LERP_DISTANCE_FACTOR = 0.1;

// World chunk size in world units
export const CHUNK_WORLD_SIZE = 1000;

// Maximum number of chunk kept in memory
export const MAX_TRACKED_CHUNK_COUNT = 50;
// Maximum (axial) distance to keep chunk in memory
export const MAX_TRACKED_CHUNK_DISTANCE = 10;
// Maximum (axial) distance to keep chunks loaded without any gap
export const MAX_LOADED_CHUNK_DISTANCE = 3;
// Maximum (axial) distance to allow interaction with
export const MAX_ACTIVE_CHUNK_DISTANCE = 0;
