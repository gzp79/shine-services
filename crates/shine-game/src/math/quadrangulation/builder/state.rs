use crate::math::debug::{SvgDump, SvgDumpFile};
use std::{cell::RefCell, path::PathBuf};

/// Work buffers and debug state for building quadrangulations.
pub struct BuilderState {
    svg_dump: RefCell<SvgDumpFile>,
}

impl BuilderState {
    pub fn new() -> Self {
        Self {
            svg_dump: RefCell::new(SvgDumpFile::new(0, "")),
        }
    }

    pub fn with_debug<P: Into<PathBuf>>(mut self, verbosity: usize, path: P) -> Self {
        self.svg_dump = RefCell::new(SvgDumpFile::new(verbosity, path));
        self
    }

    pub fn dump<F>(&self, verbosity: usize, name: &str, f: F)
    where
        F: FnOnce(&mut SvgDump),
    {
        log::trace!("Dumping {name}");
        let mut svg_dump = self.svg_dump.borrow_mut();
        if let Some(mut scope) = svg_dump.scope(verbosity, name) {
            scope.add_default_styles();
            f(&mut *scope);
        };
    }
}

impl Default for BuilderState {
    fn default() -> Self {
        Self::new()
    }
}
