//! Structured Tracing - Request context with run_id, bot_id, mode
//! Provides structured logging and trace context extraction

use axum::http::HeaderMap;

/// Structured log entry with trace context
#[derive(serde::Serialize)]
pub struct TraceLog {
    pub timestamp: String,
    pub level: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bot_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

impl TraceLog {
    pub fn new(level: &str, message: &str) -> Self {
        Self {
            timestamp: chrono::Utc::now().to_rfc3339(),
            level: level.to_string(),
            message: message.to_string(),
            run_id: None,
            bot_id: None,
            mode: None,
            user_id: None,
            duration_ms: None,
        }
    }

    pub fn with_run_id(mut self, id: u64) -> Self {
        self.run_id = Some(id);
        self
    }

    pub fn with_bot_id(mut self, id: i64) -> Self {
        self.bot_id = Some(id);
        self
    }

    pub fn with_mode(mut self, mode: &str) -> Self {
        self.mode = Some(mode.to_string());
        self
    }

    pub fn with_user_id(mut self, id: i64) -> Self {
        self.user_id = Some(id);
        self
    }

    pub fn with_duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = Some(ms);
        self
    }

    /// Log to stderr as JSON
    pub fn log(&self) {
        if let Ok(json) = serde_json::to_string(self) {
            eprintln!("{}", json);
        }
    }
}

/// Trace context extracted from request headers
#[derive(Debug, Clone)]
pub struct TraceContext {
    pub run_id: Option<u64>,
    pub bot_id: Option<i64>,
    pub mode: Option<String>,
}

impl TraceContext {
    pub fn new() -> Self {
        Self {
            run_id: None,
            bot_id: None,
            mode: None,
        }
    }

    pub fn with_run_id(mut self, id: u64) -> Self {
        self.run_id = Some(id);
        self
    }

    pub fn with_bot_id(mut self, id: i64) -> Self {
        self.bot_id = Some(id);
        self
    }

    pub fn with_mode(mut self, mode: &str) -> Self {
        self.mode = Some(mode.to_string());
        self
    }
}

impl Default for TraceContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract tracing headers from request
pub fn extract_trace_context(headers: &HeaderMap) -> TraceContext {
    let run_id = headers
        .get("X-Run-Id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok());

    let bot_id = headers
        .get("X-Bot-Id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok());

    let mode = headers
        .get("X-Trading-Mode")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    TraceContext {
        run_id,
        bot_id,
        mode,
    }
}

/// Generate a new run_id (nanosecond timestamp mixed with random)
pub fn generate_run_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    let random: u64 = rand::random();
    now.wrapping_mul(14695981039346656037u64).wrapping_add(random)
}
