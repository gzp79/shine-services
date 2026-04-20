use crate::math::triangulation::Triangulation;

pub struct Validator<'a, const DELAUNAY: bool> {
    pub(super) tri: &'a Triangulation<DELAUNAY>,
}

impl<'a, const DELAUNAY: bool> Validator<'a, DELAUNAY> {
    pub fn new(tri: &'a Triangulation<DELAUNAY>) -> Self {
        Validator { tri }
    }

    pub fn validate(&self) -> Result<(), String> {
        self.validate_topology()?;
        self.validate_geometry()?;
        Ok(())
    }
}

impl<const DELAUNAY: bool> Triangulation<DELAUNAY> {
    pub fn validator(&self) -> Validator<'_, DELAUNAY> {
        Validator::new(self)
    }

    pub fn validate(&self) -> Result<(), String> {
        self.validator().validate()
    }
}
