use bevy::{
    math::Vec2,
    render::camera::{Camera, ViewportConversionError},
};

pub trait CameraExt {
    fn viewport_to_ndc(&self, position: Vec2) -> Result<Vec2, ViewportConversionError>;
    fn viewport_to_screen_centered(&self, position: Vec2) -> Result<Vec2, ViewportConversionError>;
}

impl CameraExt for Camera {
    fn viewport_to_ndc(&self, viewport_position: Vec2) -> Result<Vec2, ViewportConversionError> {
        let target_rect = self
            .logical_viewport_rect()
            .ok_or(ViewportConversionError::NoViewportSize)?;
        let mut rect_relative = (viewport_position - target_rect.min) / target_rect.size();
        // Flip the Y co-ordinate origin from the top to the bottom.
        rect_relative.y = 1.0 - rect_relative.y;

        let ndc = rect_relative * 2. - Vec2::ONE;
        Ok(ndc)
    }

    fn viewport_to_screen_centered(&self, position: Vec2) -> Result<Vec2, ViewportConversionError> {
        let target_rect = self
            .logical_viewport_rect()
            .ok_or(ViewportConversionError::NoViewportSize)?;

        Ok(Vec2::new(
            position.x - target_rect.width() / 2.0,
            target_rect.height() / 2.0 - position.y,
        ))
    }
}
