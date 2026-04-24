use lazy_static::lazy_static;
use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounterVec, Opts, Registry, TextEncoder,
};

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();

    /// Total tool call count by tool name and status.
    pub static ref TOOL_CALLS: IntCounterVec = IntCounterVec::new(
        Opts::new("cctraveler_tool_calls_total", "Total number of tool calls"),
        &["tool", "status"],
    )
    .unwrap();

    /// Tool call latency in seconds.
    pub static ref TOOL_LATENCY: HistogramVec = HistogramVec::new(
        HistogramOpts::new("cctraveler_tool_latency_seconds", "Tool call latency")
            .buckets(vec![0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0]),
        &["tool"],
    )
    .unwrap();

    /// Cache hit/miss counter by cache layer and transport type.
    pub static ref CACHE_HITS: IntCounterVec = IntCounterVec::new(
        Opts::new("cctraveler_cache_hits_total", "Cache hit count"),
        &["layer", "transport_type"],
    )
    .unwrap();

    /// Active price subscriptions gauge.
    pub static ref ACTIVE_SUBSCRIPTIONS: IntCounterVec = IntCounterVec::new(
        Opts::new("cctraveler_subscriptions_total", "Price subscription events"),
        &["action"],
    )
    .unwrap();
}

/// Register all metrics with the global registry.
/// Call once at startup.
pub fn init_metrics() {
    REGISTRY.register(Box::new(TOOL_CALLS.clone())).ok();
    REGISTRY.register(Box::new(TOOL_LATENCY.clone())).ok();
    REGISTRY.register(Box::new(CACHE_HITS.clone())).ok();
    REGISTRY
        .register(Box::new(ACTIVE_SUBSCRIPTIONS.clone()))
        .ok();
}

/// Render all metrics as Prometheus text exposition format.
pub fn render_metrics() -> String {
    let encoder = TextEncoder::new();
    let families = REGISTRY.gather();
    let mut buffer = Vec::new();
    encoder.encode(&families, &mut buffer).ok();
    String::from_utf8(buffer).unwrap_or_default()
}

/// Record a tool call (success or error).
pub fn record_tool_call(tool_name: &str, success: bool, duration_secs: f64) {
    let status = if success { "ok" } else { "error" };
    TOOL_CALLS.with_label_values(&[tool_name, status]).inc();
    TOOL_LATENCY
        .with_label_values(&[tool_name])
        .observe(duration_secs);
}

/// Record a cache hit or miss.
pub fn record_cache_event(layer: &str, transport_type: &str) {
    CACHE_HITS
        .with_label_values(&[layer, transport_type])
        .inc();
}
