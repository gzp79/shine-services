use godot::classes::{INode3D, Node3D};
use godot::prelude::*;

#[derive(GodotClass)]
#[class(base=Node3D)]
struct Chunk {
    #[export]
    name: GString,
}

#[godot_api]
impl INode3D for Chunk {
    fn init(base: Base<Node3D>) -> Self {
        godot_print!("Hello, world!"); // Prints to the Godot console

        Self { name: "hello".into() }
    }

    fn ready(&mut self) {
        godot_print!("Chunk is ready, name: {}", self.name);
    }
}
