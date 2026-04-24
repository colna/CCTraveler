use crate::notifier::{NotifyChannel, Notifier, PriceAlert};
use crate::scrape::{scrape_flights, scrape_trains};
use storage::Database;
use tracing::{info, warn};

/// Background price check scheduler.
///
/// Periodically checks all active price subscriptions and sends alerts
/// (via webhook and/or log) when current prices fall below the user's threshold.
pub struct PriceScheduler {
    db_path: std::path::PathBuf,
    scraper_base_url: String,
    interval_seconds: u64,
    notifier: Notifier,
}

impl PriceScheduler {
    pub fn new(
        db_path: std::path::PathBuf,
        scraper_base_url: String,
        interval_seconds: u64,
    ) -> Self {
        Self {
            db_path,
            scraper_base_url,
            interval_seconds,
            notifier: Notifier::log_only(),
        }
    }

    /// Configure webhook notification delivery.
    pub fn with_webhooks(mut self, urls: Vec<String>) -> Self {
        let mut channels: Vec<NotifyChannel> = urls
            .into_iter()
            .map(|url| NotifyChannel::Webhook { url })
            .collect();
        channels.push(NotifyChannel::LogOnly);
        self.notifier = Notifier::new(channels);
        self
    }

    /// Spawn the scheduler as a background tokio task.
    /// Returns a `JoinHandle` that can be used to cancel the task.
    pub fn spawn(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            info!(
                "Price scheduler started (interval={}s)",
                self.interval_seconds
            );
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(self.interval_seconds)).await;
                self.check_all_subscriptions().await;
            }
        })
    }

    async fn check_all_subscriptions(&self) {
        let db = match Database::open(&self.db_path) {
            Ok(db) => db,
            Err(e) => {
                warn!("Scheduler: failed to open DB: {e}");
                return;
            }
        };

        let subs = match db.list_active_subscriptions(None) {
            Ok(s) => s,
            Err(e) => {
                warn!("Scheduler: failed to list subscriptions: {e}");
                return;
            }
        };

        if subs.is_empty() {
            return;
        }

        info!("Scheduler: checking {} active subscriptions", subs.len());
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

        for sub in &subs {
            let from = sub["from_city"].as_str().unwrap_or("");
            let to = sub["to_city"].as_str().unwrap_or("");
            let transport = sub["transport_type"].as_str().unwrap_or("train");
            let threshold = sub["threshold"].as_f64().unwrap_or(0.0);
            let sub_id = sub["id"].as_str().unwrap_or("");
            let expires = sub["expires_at"].as_str().unwrap_or("");

            // Check if expired
            if expires < today.as_str() {
                info!("Subscription {sub_id} expired, deactivating");
                let _ = db.deactivate_subscription(sub_id);
                continue;
            }

            let cheapest = match transport {
                "train" => self.get_cheapest_train(from, to, &today).await,
                "flight" => self.get_cheapest_flight(from, to, &today).await,
                _ => None,
            };

            if let Some(price) = cheapest {
                if price <= threshold {
                    let alert = PriceAlert {
                        subscription_id: sub_id.to_string(),
                        from_city: from.to_string(),
                        to_city: to.to_string(),
                        transport_type: transport.to_string(),
                        current_price: price,
                        threshold,
                        message: format!(
                            "当前最低价 ¥{:.0} 已低于您设定的 ¥{:.0}，建议尽快购票！",
                            price, threshold
                        ),
                    };
                    self.notifier.send_alert(&alert).await;
                }
            }
        }
    }

    async fn get_cheapest_train(&self, from: &str, to: &str, date: &str) -> Option<f64> {
        // Try DB cache first
        let db = Database::open(&self.db_path).ok()?;
        let cached = db.search_trains(from, to, date, 120).ok()?;
        if !cached.is_empty() {
            return cached.iter().filter_map(|r| r.lowest_price).reduce(f64::min);
        }

        // Try live scrape
        let trains = scrape_trains(&self.scraper_base_url, from, to, date)
            .await
            .ok()?;
        trains
            .iter()
            .flat_map(|t| t.seats.iter().map(|s| s.price))
            .reduce(f64::min)
    }

    async fn get_cheapest_flight(&self, from: &str, to: &str, date: &str) -> Option<f64> {
        let db = Database::open(&self.db_path).ok()?;
        let cached = db.search_flights(from, to, date, 120).ok()?;
        if !cached.is_empty() {
            return cached.iter().filter_map(|r| r.lowest_price).reduce(f64::min);
        }

        let flights = scrape_flights(&self.scraper_base_url, from, to, date)
            .await
            .ok()?;
        flights
            .iter()
            .flat_map(|f| f.prices.iter().map(|p| p.price))
            .reduce(f64::min)
    }
}
