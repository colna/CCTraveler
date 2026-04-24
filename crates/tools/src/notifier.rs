use serde::Serialize;
use tracing::{info, warn};

/// Price alert notification that gets sent when a subscription triggers.
#[derive(Debug, Serialize)]
pub struct PriceAlert {
    pub subscription_id: String,
    pub from_city: String,
    pub to_city: String,
    pub transport_type: String,
    pub current_price: f64,
    pub threshold: f64,
    pub message: String,
}

/// Notification delivery channel.
#[derive(Debug, Clone)]
pub enum NotifyChannel {
    /// Send alerts to a webhook URL (Feishu, Slack, Discord, generic HTTP POST).
    Webhook { url: String },
    /// Log-only mode (no external delivery).
    LogOnly,
}

/// Notifier handles delivering price alerts to configured channels.
pub struct Notifier {
    channels: Vec<NotifyChannel>,
    client: reqwest::Client,
}

impl Notifier {
    pub fn new(channels: Vec<NotifyChannel>) -> Self {
        Self {
            channels,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Create a log-only notifier (no external channels).
    pub fn log_only() -> Self {
        Self::new(vec![NotifyChannel::LogOnly])
    }

    /// Create a notifier with a webhook channel.
    pub fn with_webhook(url: String) -> Self {
        let mut channels = vec![NotifyChannel::Webhook { url }];
        channels.push(NotifyChannel::LogOnly);
        Self::new(channels)
    }

    /// Send a price alert to all configured channels.
    pub async fn send_alert(&self, alert: &PriceAlert) {
        for channel in &self.channels {
            match channel {
                NotifyChannel::Webhook { url } => {
                    self.send_webhook(url, alert).await;
                }
                NotifyChannel::LogOnly => {
                    info!(
                        "PRICE ALERT: {} → {} ({}) ¥{:.0} <= ¥{:.0} [sub={}]",
                        alert.from_city,
                        alert.to_city,
                        alert.transport_type,
                        alert.current_price,
                        alert.threshold,
                        alert.subscription_id,
                    );
                }
            }
        }
    }

    async fn send_webhook(&self, url: &str, alert: &PriceAlert) {
        let payload = serde_json::json!({
            "msg_type": "text",
            "content": {
                "text": format!(
                    "🔔 价格提醒\n{} → {} ({})\n当前价格: ¥{:.0}\n目标价格: ¥{:.0}\n{}",
                    alert.from_city,
                    alert.to_city,
                    alert.transport_type,
                    alert.current_price,
                    alert.threshold,
                    alert.message,
                )
            },
            "alert": alert,
        });

        match self.client.post(url).json(&payload).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    info!(
                        "Webhook notification sent for subscription {}",
                        alert.subscription_id
                    );
                } else {
                    warn!(
                        "Webhook returned {}: {}",
                        resp.status(),
                        resp.text().await.unwrap_or_default()
                    );
                }
            }
            Err(e) => {
                warn!("Failed to send webhook notification: {e}");
            }
        }
    }
}
