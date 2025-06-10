extends Node3D

const Constants = preload("res://constants.gd")
const CHUNK_SIZE := Constants.CHUNK_SIZE

var root = Vector2i.ZERO;
@onready var camera: Camera3D = $"../Camera"

func _ready():
	camera.connect("request_world_origin_shift", self._on_world_origin_changed)


func _on_world_origin_changed(new_origin: Vector2) -> void:
	var grid_offset = Vector2i(floor(new_origin / CHUNK_SIZE))
	var world_offset = Vector3(grid_offset.x, 0, grid_offset.y);

	root += grid_offset;

	print_debug("New root: ", root)
	
	camera.on_origin_shift(-world_offset);
	# shift all the children of this node in the opposite direction
	for child in get_children():
		if child is Node3D:
			child.position -= world_offset
