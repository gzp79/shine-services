/// Error type for CDT triangulation.
#[derive(Debug, Eq, PartialEq)]
pub enum CdtError {
    PointOnFixedEdge(usize),
    NoMorePoints,
    CrossingFixedEdge,
    EmptyInput,
    InvalidInput,
    InvalidEdge,
    OpenContour,
    TooFewPoints,
    CannotInitialize,
    WedgeEscape,
}

impl std::fmt::Display for CdtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CdtError::PointOnFixedEdge(i) => write!(f, "point {i} is on a fixed edge"),
            CdtError::NoMorePoints => write!(f, "no more points"),
            CdtError::CrossingFixedEdge => write!(f, "fixed edges cross"),
            CdtError::EmptyInput => write!(f, "empty input"),
            CdtError::InvalidInput => write!(f, "invalid input"),
            CdtError::InvalidEdge => write!(f, "invalid edge"),
            CdtError::OpenContour => write!(f, "open contour"),
            CdtError::TooFewPoints => write!(f, "too few points"),
            CdtError::CannotInitialize => write!(f, "cannot initialize"),
            CdtError::WedgeEscape => write!(f, "escaped wedge"),
        }
    }
}

impl std::error::Error for CdtError {}
