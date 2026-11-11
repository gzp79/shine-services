use std::backtrace::Backtrace;
use tracing::{span, Span, Subscriber};
use tracing_opentelemetry::OpenTelemetrySpanExt as _;
use tracing_subscriber::{layer::Context, registry::LookupSpan, Layer};

/// Name of the trace attribute holding the stack of a root span (OpenTelemetry semantic convention).
const STACKTRACE_ATTRIBUTE: &str = "code.stacktrace";

/// Adds the call stack that created the span as the `code.stacktrace` attribute of every span
/// created without a parent, to find what starts the noisy single-span traces. Debug aid (see
/// `telemetry.rootSpanOrigin`), not meant for production.
///
/// Costs one stack capture per root span and, when the span is first entered, the resolution of
/// its symbols; child spans cost a parent lookup only. The attribute can only be set on the
/// current span, so it is added on the first enter, not on creation. It lands in the trace only,
/// and only where an OpenTelemetry layer is installed before this one.
#[derive(Default)]
pub struct RootSpanOriginLayer;

impl RootSpanOriginLayer {
    pub fn new() -> Self {
        Self
    }
}

impl<S> Layer<S> for RootSpanOriginLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        // The lookup follows the filter of the sinks this layer is attached to, so "root" means
        // what the trace backend sees as a root.
        let is_root = attrs.is_root() || (attrs.is_contextual() && ctx.lookup_current().is_none());
        if is_root {
            if let Some(span) = ctx.span(id) {
                span.extensions_mut().insert(PendingOrigin(Backtrace::force_capture()));
            }
        }
    }

    fn on_enter(&self, id: &span::Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            return;
        };
        // The extensions lock is released before the attribute is set, which locks it again.
        let origin = span.extensions_mut().remove::<PendingOrigin>();
        let Some(PendingOrigin(backtrace)) = origin else {
            return;
        };
        // On enter the span is the current one, the only handle the attribute can be set through.
        let current = Span::current();
        if current.id().as_ref() == Some(id) {
            current.set_attribute(STACKTRACE_ATTRIBUTE, trim_tracing_frames(&backtrace));
        }
    }
}

/// Stack captured when a root span was created, waiting for the span to be entered.
struct PendingOrigin(Backtrace);

/// Drops the leading frames that belong to the capture itself (std, this layer, tracing), so the
/// first frame is the code that created the span.
fn trim_tracing_frames(backtrace: &Backtrace) -> String {
    // The capture itself: std, this layer, and the tracing machinery between it and the span's creator.
    // Tracing frames can be wrapped in std ones (thread-local access), so they are matched anywhere in the symbol.
    const CAPTURE_PREFIXES: &[&str] = &["std::backtrace", "shine_infra::telemetry::root_span_origin::impl"];
    const TRACING_PARTS: &[&str] = &[
        "tracing_core::",
        "tracing_subscriber::",
        "tracing_opentelemetry::",
        "tracing::span::",
        "tracing::dispatcher::",
    ];

    let text = backtrace.to_string();
    // A frame is a numbered header line followed by its `at file:line` lines.
    let mut frames: Vec<Vec<&str>> = Vec::new();
    for line in text.lines() {
        let is_header = line
            .trim_start()
            .split(':')
            .next()
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit()));
        match frames.last_mut() {
            Some(frame) if !is_header => frame.push(line),
            _ => frames.push(vec![line]),
        }
    }
    frames
        .into_iter()
        .skip_while(|frame| {
            // "  12: symbol"
            let symbol = frame[0].split_once(": ").map_or("", |(_, symbol)| symbol);
            CAPTURE_PREFIXES.iter().any(|prefix| symbol.starts_with(prefix))
                || TRACING_PARTS.iter().any(|part| symbol.contains(part))
        })
        .flatten()
        .collect::<Vec<_>>()
        .join("\n")
}
