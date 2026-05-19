use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use tracing::Level;
use tracing_subscriber::{Layer, filter::LevelFilter, layer::Context};

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
}

pub struct LogBuffer {
    entries: Mutex<VecDeque<LogEntry>>,
    max_entries: usize,
}

impl LogBuffer {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Mutex::new(VecDeque::with_capacity(max_entries)),
            max_entries,
        }
    }

    pub fn entries(&self) -> Vec<LogEntry> {
        self.entries.lock().unwrap().iter().cloned().collect()
    }

    pub fn clear(&self) {
        self.entries.lock().unwrap().clear();
    }

    fn push(&self, entry: LogEntry) {
        let mut entries = self.entries.lock().unwrap();
        if entries.len() >= self.max_entries {
            entries.pop_front();
        }
        entries.push_back(entry);
    }
}

pub struct UiLogLayer {
    buffer: Arc<LogBuffer>,
}

impl UiLogLayer {
    pub fn new(buffer: Arc<LogBuffer>) -> Self {
        Self { buffer }
    }
}

impl<S> Layer<S> for UiLogLayer
where
    S: tracing::Subscriber,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| {
                let secs = d.as_secs();
                let hours = (secs % 86400) / 3600;
                let mins = (secs % 3600) / 60;
                let secs = secs % 60;
                format!("{:02}:{:02}:{:02}", hours, mins, secs)
            })
            .unwrap_or_default();

        let level = match *event.metadata().level() {
            Level::ERROR => "ERROR",
            Level::WARN => "WARN ",
            Level::INFO => "INFO ",
            Level::DEBUG => "DEBUG",
            Level::TRACE => "TRACE",
        };

        let mut message = String::new();
        let mut visitor = MessageVisitor(&mut message);
        event.record(&mut visitor);

        let entry = LogEntry {
            timestamp,
            level: level.to_string(),
            message,
        };

        self.buffer.push(entry);
    }
}

struct MessageVisitor<'a>(&'a mut String);

impl tracing::field::Visit for MessageVisitor<'_> {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if !self.0.is_empty() {
            self.0.push(' ');
        }
        if field.name() == "message" {
            self.0.push_str(&format!("{:?}", value));
        } else {
            self.0.push_str(&format!("{}={:?}", field.name(), value));
        }
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if !self.0.is_empty() {
            self.0.push(' ');
        }
        if field.name() == "message" {
            self.0.push_str(value);
        } else {
            self.0.push_str(&format!("{}={}", field.name(), value));
        }
    }
}

pub fn init(buffer: Arc<LogBuffer>) {
    use tracing_subscriber::{prelude::*, registry::Registry};

    let ui_layer = UiLogLayer::new(buffer.clone()).with_filter(LevelFilter::TRACE);

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_level(true)
        .with_filter(LevelFilter::DEBUG);

    Registry::default().with(ui_layer).with(fmt_layer).init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_buffer_new() {
        let buffer = LogBuffer::new(100);
        assert!(buffer.entries().is_empty());
    }

    #[test]
    fn test_log_buffer_push_and_entries() {
        let buffer = LogBuffer::new(10);
        buffer.push(LogEntry {
            timestamp: "00:00:00".to_string(),
            level: "INFO ".to_string(),
            message: "test message".to_string(),
        });
        let entries = buffer.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, "test message");
    }

    #[test]
    fn test_log_buffer_clear() {
        let buffer = LogBuffer::new(10);
        buffer.push(LogEntry {
            timestamp: "00:00:00".to_string(),
            level: "INFO ".to_string(),
            message: "test".to_string(),
        });
        buffer.clear();
        assert!(buffer.entries().is_empty());
    }

    #[test]
    fn test_log_buffer_max_entries() {
        let buffer = LogBuffer::new(3);
        for i in 0..5 {
            buffer.push(LogEntry {
                timestamp: "00:00:00".to_string(),
                level: "INFO ".to_string(),
                message: format!("msg {}", i),
            });
        }
        let entries = buffer.entries();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].message, "msg 2");
        assert_eq!(entries[2].message, "msg 4");
    }

    #[test]
    fn test_log_entry_clone() {
        let entry = LogEntry {
            timestamp: "12:00:00".to_string(),
            level: "ERROR".to_string(),
            message: "something broke".to_string(),
        };
        let cloned = entry.clone();
        assert_eq!(cloned.message, entry.message);
    }
}
