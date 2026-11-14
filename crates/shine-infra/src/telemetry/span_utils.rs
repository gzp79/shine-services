use tracing::Span;

/// Builds the span only when there is an active parent span; otherwise returns a disabled span.
/// Keeps background work (polling loops, startup tasks) from producing one-span root traces.
///
/// Takes a closure because span macros need their callsite at the place they are written.
pub fn child_span(make: impl FnOnce() -> Span) -> Span {
    if Span::current().is_none() {
        Span::none()
    } else {
        make()
    }
}
