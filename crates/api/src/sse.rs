/// SSE (Server-Sent Events) frame parser for streaming LLM responses.

#[derive(Debug, Clone)]
pub struct SseEvent {
    pub event_type: String,
    pub data: String,
}

/// Parse a full SSE response body into individual events.
#[must_use] 
pub fn parse_sse_body(body: &str) -> Vec<SseEvent> {
    let mut events = Vec::new();
    let mut current_event = String::new();
    let mut current_data = String::new();

    for line in body.lines() {
        if let Some(event_type) = line.strip_prefix("event: ") {
            current_event = event_type.trim().to_string();
        } else if let Some(data) = line.strip_prefix("data: ") {
            current_data = data.to_string();
        } else if line.is_empty() && !current_data.is_empty() {
            events.push(SseEvent {
                event_type: std::mem::take(&mut current_event),
                data: std::mem::take(&mut current_data),
            });
        }
    }

    // Handle final event if no trailing newline
    if !current_data.is_empty() {
        events.push(SseEvent {
            event_type: current_event,
            data: current_data,
        });
    }

    events
}

/// Parse SSE from a streaming reader (line by line).
pub fn parse_sse_lines<I: Iterator<Item = String>>(lines: I) -> Vec<SseEvent> {
    let mut events = Vec::new();
    let mut current_event = String::new();
    let mut current_data = String::new();

    for line in lines {
        if let Some(event_type) = line.strip_prefix("event: ") {
            current_event = event_type.trim().to_string();
        } else if let Some(data) = line.strip_prefix("data: ") {
            current_data = data.to_string();
        } else if line.is_empty() && !current_data.is_empty() {
            events.push(SseEvent {
                event_type: std::mem::take(&mut current_event),
                data: std::mem::take(&mut current_data),
            });
        }
    }

    if !current_data.is_empty() {
        events.push(SseEvent {
            event_type: current_event,
            data: current_data,
        });
    }

    events
}
