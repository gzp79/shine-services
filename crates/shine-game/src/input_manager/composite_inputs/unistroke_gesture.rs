

/// Unistroke gesture recognizer for the given input points.
/// Recognizer incorporates all the points in a Some(Vec2) sequence and triggers 
/// when a gesture is recognized.
pub struct UnistrokeGesture<A:ActionLike> 
where P: DualAxisLike
{
    input_action: A,
    pub points: Vec<Vec2>,
}