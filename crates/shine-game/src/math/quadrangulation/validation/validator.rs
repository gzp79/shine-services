use crate::math::quadrangulation::{QuadError, QuadTopology};

pub struct Validator<'a> {
    pub(super) topology: &'a QuadTopology,
}

impl<'a> Validator<'a> {
    pub fn new(topology: &'a QuadTopology) -> Self {
        Validator { topology }
    }

    pub fn validate(&self) -> Result<(), QuadError> {
        self.validate_topology()?;
        Ok(())
    }
}
