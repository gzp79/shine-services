use bevy::{
    app::{App, PreUpdate, Update},
    ecs::{component::Component, resource::Resource, schedule::IntoScheduleConfigs},
};
use serde::{Deserialize, Serialize};
use shine_game::map::{
    create_layer_system, process_layer_commands_system, process_map_event_system,
    remove_layer_system, ChunkCommandQueue, ChunkEvent, ChunkHashTrack, ChunkHasher, ChunkLayer,
    ChunkOperation, GridConfig, LayerSetup, MapChunk, MapConfig,
};

#[derive(Resource, Clone)]
pub struct TestGridConfig {
    pub width: usize,
    pub height: usize,
}

impl MapConfig for TestGridConfig {}

impl GridConfig for TestGridConfig {
    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestDataOperation {
    pub value: usize,
}

impl ChunkOperation<TestData> for TestDataOperation {
    fn check_precondition(&self, _chunk: &TestData) -> bool {
        true
    }

    fn apply(self, chunk: &mut TestData) {
        chunk.data = Some(self.value);
    }
}

#[derive(Component, Debug)]
pub struct TestData {
    data: Option<usize>,
}

impl TestData {
    pub fn data(&self) -> Option<usize> {
        self.data
    }
}

impl From<TestGridConfig> for TestData {
    fn from(config: TestGridConfig) -> Self {
        Self {
            data: Some(config.width * config.height),
        }
    }
}

impl MapChunk for TestData {
    fn name() -> &'static str {
        "TestData"
    }

    fn new_empty() -> Self {
        Self { data: None }
    }

    fn is_empty(&self) -> bool {
        self.data.is_none()
    }
}

#[derive(Resource, Default, Clone)]
pub struct TestDataHasher;

impl ChunkHasher<TestData> for TestDataHasher {
    type Hash = usize;

    fn hash(&self, chunk: &TestData) -> Self::Hash {
        chunk.data.unwrap_or(0)
    }
}

pub type TestDataHashTracker = ChunkHashTrack<TestData, TestDataHasher>;
pub type TestDataLayer = ChunkLayer<TestData>;
pub type TestDataLayerEvent = ChunkEvent<TestData>;

#[derive(Clone)]
pub struct TestDataLayerSetup {
    pub command_queue: ChunkCommandQueue<TestData, TestDataOperation, TestDataHasher>,
}

impl Default for TestDataLayerSetup {
    fn default() -> Self {
        Self::new()
    }
}

impl TestDataLayerSetup {
    pub fn new() -> Self {
        Self {
            command_queue: ChunkCommandQueue::new(),
        }
    }

    pub fn new_with_queue(
        command_queue: ChunkCommandQueue<TestData, TestDataOperation, TestDataHasher>,
    ) -> Self {
        Self { command_queue }
    }
}

impl LayerSetup<TestGridConfig> for TestDataLayerSetup {
    fn build(&self, app: &mut App) {
        log::debug!("Adding map layer: {}", TestData::name());
        app.insert_resource(TestDataHasher);
        app.insert_resource(self.command_queue.clone());
        app.insert_resource(ChunkLayer::<TestData>::new());

        app.add_event::<TestDataLayerEvent>();

        app.add_systems(
            PreUpdate,
            (
                create_layer_system::<TestData, TestDataHasher>,
                remove_layer_system::<TestData>,
            )
                .chain()
                .after(process_map_event_system),
        );

        app.add_systems(
            Update,
            process_layer_commands_system::<
                TestGridConfig,
                TestData,
                TestDataOperation,
                TestDataHasher,
            >,
        );
    }
}
