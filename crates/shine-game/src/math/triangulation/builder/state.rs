use crate::math::debug::{SvgDump, SvgDumpFile};
use crate::math::triangulation::{FaceEdge, Triangulation};
use std::{cell::RefCell, path::PathBuf};

/// Work buffers and state for building triangulations.
///
/// BuilderState holds reusable buffers and debug state that algorithms need
/// but that aren't part of the core triangulation data structure.
pub struct BuilderState {
    /// Delaunay stack - None when locked for processing
    delaunay_stack: Option<Vec<FaceEdge>>,
    /// Constraint chains (edge, top, bottom)
    constraint_chains: Option<(Vec<FaceEdge>, Vec<FaceEdge>, Vec<FaceEdge>)>,
    /// SVG dump state for debugging and visualization
    svg_dump: RefCell<SvgDumpFile>,
}

impl BuilderState {
    pub fn new() -> Self {
        Self {
            delaunay_stack: Some(Vec::new()),
            constraint_chains: Some((Vec::new(), Vec::new(), Vec::new())),
            svg_dump: RefCell::new(SvgDumpFile::new(0, "")),
        }
    }

    pub fn with_debug<P: Into<PathBuf>>(mut self, verbosity: usize, path: P) -> Self {
        self.svg_dump = RefCell::new(SvgDumpFile::new(verbosity, path));
        self
    }

    /// Execute a closure with a dump scope if enabled, with default styles pre-applied.
    ///
    /// This method takes a mutable reference to svg_dump to avoid borrowing the entire state,
    /// allowing the closure to access other fields like delaunay_stack, top_chain, etc.
    pub fn dump<F>(&self, verbosity: usize, name: &str, f: F)
    where
        F: FnOnce(&mut SvgDump),
    {
        log::trace!("Dumping {name}");

        //let svg_dump = self.svg_dump.clone();
        let mut svg_dump = self.svg_dump.borrow_mut();

        if let Some(mut scope) = svg_dump.scope(verbosity, name) {
            scope.add_default_styles();

            // Add builder-specific styles
            scope.add_style(
                "edge-delaunay",
                "stroke: #ff6b6b; stroke-width: 2.5px; stroke-linecap: round; vector-effect: non-scaling-stroke;",
            );
            scope.add_style(
                "edge-0",
                "stroke: #3498db; stroke-width: 2.5px; stroke-linecap: round; vector-effect: non-scaling-stroke;",
            );
            scope.add_style(
                "edge-1",
                "stroke: #f39c12; stroke-width: 2.5px; stroke-linecap: round; vector-effect: non-scaling-stroke;",
            );
            scope.add_style(
                "edge-2",
                "stroke: #9b59b6; stroke-width: 2.5px; stroke-linecap: round; vector-effect: non-scaling-stroke;",
            );

            f(&mut *scope);
        };
    }

    #[inline]
    pub fn delaunay_push_edge_into<const DELAUNAY: bool>(
        stack: &mut Vec<FaceEdge>,
        tri: &Triangulation<DELAUNAY>,
        edge: FaceEdge,
    ) {
        debug_assert!(DELAUNAY && tri.dimension() == 2);

        let twin = tri.twin_edge(edge);
        if !stack.contains(&edge) && !stack.contains(&twin) {
            log::trace!("Adding to delaunay: {edge:?}");
            stack.push(edge);
        }
    }

    /// Push an edge to the delaunay stack if it's not already present.
    /// Used to enqueue edges for delaunay triangulation checking.
    #[inline]
    pub fn delaunay_push_edge<const DELAUNAY: bool>(&mut self, tri: &Triangulation<DELAUNAY>, edge: FaceEdge) {
        if !DELAUNAY || tri.dimension() != 2 {
            return;
        }

        let stack = self
            .delaunay_stack
            .as_mut()
            .expect("Delaunay stack is locked for processing");
        Self::delaunay_push_edge_into(stack, tri, edge);
    }

    /// Lock the delaunay stack for processing, returning ownership of the vector.
    /// The stack remains locked (None) until unlock_delaunay_stack is called.
    /// Panics if the stack is already locked.
    pub fn lock_delaunay_stack(&mut self) -> Vec<FaceEdge> {
        self.delaunay_stack.take().expect("Delaunay stack is already locked")
    }

    /// Unlock the delaunay stack, returning it to available state.
    pub fn unlock_delaunay_stack(&mut self, stack: Vec<FaceEdge>) {
        self.delaunay_stack = Some(stack);
    }

    /// Get a reference to the delaunay stack if it's not locked.
    pub fn delaunay_stack(&self) -> Option<&Vec<FaceEdge>> {
        self.delaunay_stack.as_ref()
    }

    /// Clear the delaunay stack if it's not locked.
    pub fn clear_delaunay_stack(&mut self) {
        if let Some(stack) = &mut self.delaunay_stack {
            stack.clear();
        }
    }

    /// Lock the constraint chains for processing, returning (edge_chain, top_chain, bottom_chain).
    /// The chains remain locked (None) until unlock_constraint_chains is called.
    /// Panics if the chains are already locked.
    pub fn lock_constraint_chains(&mut self) -> (Vec<FaceEdge>, Vec<FaceEdge>, Vec<FaceEdge>) {
        self.constraint_chains
            .take()
            .expect("Constraint chains are already locked")
    }

    /// Unlock the constraint chains, returning them to available state.
    pub fn unlock_constraint_chains(&mut self, chains: (Vec<FaceEdge>, Vec<FaceEdge>, Vec<FaceEdge>)) {
        self.constraint_chains = Some(chains);
    }
}

impl Default for BuilderState {
    fn default() -> Self {
        Self::new()
    }
}
