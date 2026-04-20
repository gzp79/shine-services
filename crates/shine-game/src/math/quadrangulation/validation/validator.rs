use crate::math::quadrangulation::{QuadError, Quadrangulation};

pub struct Validator<'a> {
    pub(super) topology: &'a Quadrangulation,
}

impl<'a> Validator<'a> {
    pub fn new(topology: &'a Quadrangulation) -> Self {
        Validator { topology }
    }

    pub fn validate(&self) -> Result<(), QuadError> {
        self.validate_topology()?;
        self.validate_geometry()?;
        Ok(())
    }
}
