use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GestureId(pub usize);

impl GestureId {
    pub fn id(&self) -> usize {
        self.0
    }
}

impl From<usize> for GestureId {
    fn from(id: usize) -> Self {
        Self(id)
    }
}

impl From<GestureId> for usize {
    fn from(gesture_id: GestureId) -> Self {
        gesture_id.id()
    }
}
