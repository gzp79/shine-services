export const InputConst = {
    MOVE_KEY_UP: 'w',
    MOVE_KEY_LEFT: 'a',
    MOVE_KEY_DOWN: 's',
    MOVE_KEY_RIGHT: 'd',
    MOVE_KEY_SPRINT: 'shift',
    MOVE_KEYS: ['w', 'a', 's', 'd', 'shift'] as readonly string[],

    ROTATE_KEY_LEFT: 'q',
    ROTATE_KEY_RIGHT: 'e',
    ROTATE_KEYS: ['q', 'e'] as readonly string[],

    ZOOM_KEY_IN: 'r',
    ZOOM_KEY_OUT: 'f',
    ZOOM_KEYS: ['r', 'f'] as readonly string[],

    TAP_THRESHOLD_MS: 500,
    LONG_PRESS_MS: 500,
    PINCH_TIMING_MS: 300,
    MOVE_THRESHOLD_PX: 6,

    ROTATE_SENSITIVITY: 0.005, // radians per pixel
    ZOOM_SENSITIVITY: 0.5 // zoom speed multiplier for mouse wheel
} as const;

export const CameraConst = {
    MIN_DISTANCE: 500,
    MAX_DISTANCE: 4000,
    MIN_PITCH: 20 * (Math.PI / 180),
    MAX_PITCH: 85 * (Math.PI / 180),
    BASE_LERP: 0.5,
    LERP_DISTANCE_FACTOR: 0.1,
    ZOOM_DISTANCE_SCALE: 8, // distance-dependent zoom scale factor

    CURSOR_MOVE_SPEED: 1200, // units per second
    CURSOR_SPRINT_MULTIPLIER: 3,
    CURSOR_ROTATE_SPEED: 90 * (Math.PI / 180), // radians/second at rate=1
    CURSOR_ZOOM_SPEED: 250 // units/second at rate=1
} as const;

export const ChunkConst = {
    WORLD_SIZE: 1000,
    MAX_TRACKED_COUNT: 50,
    MAX_TRACKED_DISTANCE: 10,
    MAX_LOADED_DISTANCE: 4,
    MAX_ACTIVE_DISTANCE: 1
} as const;
