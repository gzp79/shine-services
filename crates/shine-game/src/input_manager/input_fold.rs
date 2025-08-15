use bevy::math::Vec2;

/// Defines a strategy for combining multiple input values into a single value within an input processing pipeline.
pub trait InputValueFold<T>: Send + Sync + 'static
where
    T: Send + Sync + 'static,
{
    fn fold(&self, rev: Option<T>, current: Option<T>) -> Option<T>;
}

/// Fold input values by taking the first non-None value.
pub struct FoldFirst;

impl<T> InputValueFold<T> for FoldFirst
where
    T: Send + Sync + 'static,
{
    fn fold(&self, prev: Option<T>, current: Option<T>) -> Option<T> {
        if let Some(v) = prev {
            Some(v)
        } else {
            current
        }
    }
}

/// Fold bool values by taking the logical AND of the inputs.
pub struct AndFold;

impl InputValueFold<bool> for AndFold {
    fn fold(&self, prev: Option<bool>, current: Option<bool>) -> Option<bool> {
        match (prev, current) {
            (Some(true), Some(true)) => Some(true),
            (Some(true), None) | (None, Some(true)) => Some(true),
            _ => None,
        }
    }
}

/// Fold vlues by taking the input with the bigger norm.
pub struct MaxFold;

impl InputValueFold<f32> for MaxFold {
    fn fold(&self, prev: Option<f32>, current: Option<f32>) -> Option<f32> {
        match (prev, current) {
            (Some(v1), Some(v2)) => {
                if v1.abs() >= v2.abs() {
                    Some(v1)
                } else {
                    Some(v2)
                }
            }
            (Some(v), None) | (None, Some(v)) => Some(v),
            (None, None) => None,
        }
    }
}

impl InputValueFold<Vec2> for MaxFold {
    fn fold(&self, prev: Option<Vec2>, current: Option<Vec2>) -> Option<Vec2> {
        match (prev, current) {
            (Some(v1), Some(v2)) => {
                if v1.length_squared() >= v2.length_squared() {
                    Some(v1)
                } else {
                    Some(v2)
                }
            }
            (Some(v), None) | (None, Some(v)) => Some(v),
            (None, None) => None,
        }
    }
}
