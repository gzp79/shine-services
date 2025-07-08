use bevy::tasks::{AsyncComputeTaskPool, TaskPool};
use std::sync::Once;

static INIT: Once = Once::new();

pub fn test_init_bevy() {
    INIT.call_once(|| {
        AsyncComputeTaskPool::get_or_init(TaskPool::new);
    });
}
