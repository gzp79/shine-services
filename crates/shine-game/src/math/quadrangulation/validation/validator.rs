use crate::math::quadrangulation::{QuadError, Quadrangulation};

pub struct Validator<'a> {
    pub(super) mesh: &'a Quadrangulation,
}

impl<'a> Validator<'a> {
    pub fn new(mesh: &'a Quadrangulation) -> Self {
        Validator { mesh }
    }

    pub fn validate(&self) -> Result<(), QuadError> {
        self.validate_topology()?;
        self.validate_geometry()?;
        Ok(())
    }
}

impl Quadrangulation {
    pub fn validator(&self) -> Validator<'_> {
        Validator::new(self)
    }

    pub fn validate(&self) -> Result<(), QuadError> {
        self.validator().validate()
    }
}
