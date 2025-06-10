extends Camera3D

const MOUSE_SENSITIVITY := 0.002

const Constants = preload("res://constants.gd")
const MAX_CHUNK_DISTANCE := Constants.MAX_CHUNK_DISTANCE

signal request_world_origin_shift(new_origin: Vector2);

var speed = 2.0;

func _ready():
	Input.mouse_mode = Input.MOUSE_MODE_CAPTURED

func _process(delta: float) -> void:
	var dir = Vector3.ZERO;
	dir.x -= Input.get_action_strength("move_left");
	dir.x += Input.get_action_strength("move_right");
	dir.z -= Input.get_action_strength("move_forward");
	dir.z += Input.get_action_strength("move_backward");
	
	dir = dir.rotated(Vector3.UP, self.rotation.y)
	self.position += dir * delta * speed;
	
	var origin: Vector3 = get_target_position();
	var origin_vector: Vector2 = Vector2(origin.x, origin.z);
	var origin_distance: float = origin_vector.length();
	if origin_distance > MAX_CHUNK_DISTANCE:
		print_debug("Requesting world origin shift");
		request_world_origin_shift.emit(origin_vector);
	
func _input(event: InputEvent) -> void:
	if event is InputEventMouseMotion:
		self.rotate_y(-event.relative.x * MOUSE_SENSITIVITY)
		rotate_x(-event.relative.y * MOUSE_SENSITIVITY)
		rotation.x = clamp(rotation.x, -PI / 2, PI / 2)
		rotation.z = 0.0

func get_target_position() -> Vector3:
	var origin = self.global_position
	var forward = - self.global_transform.basis.z.normalized()
	
	# If forward.z == 0, the ray is parallel to the XY plane and will never intersect
	if abs(forward.z) < 0.0001:
		return origin # or null, or some indication of no intersection
	
	# t is the distance along the ray to reach z = 0
	var t = - origin.z / forward.z
	var intersection = origin + forward * t
	return intersection

func on_origin_shift(delta: Vector3) -> void:
	self.position += delta
