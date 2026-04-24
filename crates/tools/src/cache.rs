use tracing::{info, warn};

/// Redis-backed cache layer for transport query results.
///
/// Sits between the tool handlers and the scraper/DB:
///   Memory (static lookups) → Redis (hot data, TTL) → SQLite (persistent)
pub struct RedisCache {
    client: Option<redis::Client>,
    ttl_seconds: u64,
}

impl RedisCache {
    /// Create a new Redis cache. If `enabled` is false or connection fails,
    /// operates as a no-op (graceful degradation).
    pub fn new(enabled: bool, url: &str, ttl_seconds: u64) -> Self {
        if !enabled {
            info!("Redis cache disabled");
            return Self {
                client: None,
                ttl_seconds,
            };
        }

        match redis::Client::open(url) {
            Ok(client) => {
                // Test connection
                let reachable = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(async {
                        let conn = client.get_multiplexed_async_connection().await;
                        conn.is_ok()
                    })
                });
                if reachable {
                    info!("Redis cache connected: {url}");
                    Self {
                        client: Some(client),
                        ttl_seconds,
                    }
                } else {
                    warn!("Redis not reachable at {url}, falling back to SQLite-only");
                    Self {
                        client: None,
                        ttl_seconds,
                    }
                }
            }
            Err(e) => {
                warn!("Redis client creation failed: {e}, falling back to SQLite-only");
                Self {
                    client: None,
                    ttl_seconds,
                }
            }
        }
    }

    /// Check if Redis is available.
    pub fn is_available(&self) -> bool {
        self.client.is_some()
    }

    /// Build a cache key for transport queries.
    fn transport_key(transport_type: &str, from: &str, to: &str, date: &str) -> String {
        format!("{transport_type}:{from}:{to}:{date}")
    }

    /// Get cached transport query result from Redis.
    pub fn get_transport(
        &self,
        transport_type: &str,
        from: &str,
        to: &str,
        date: &str,
    ) -> Option<String> {
        let client = self.client.as_ref()?;
        let key = Self::transport_key(transport_type, from, to, date);

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let mut conn = match client.get_multiplexed_async_connection().await {
                    Ok(c) => c,
                    Err(e) => {
                        warn!("Redis GET connection failed: {e}");
                        return None;
                    }
                };
                match redis::AsyncCommands::get::<_, Option<String>>(&mut conn, &key).await {
                    Ok(val) => {
                        if val.is_some() {
                            info!("Redis cache HIT: {key}");
                        }
                        val
                    }
                    Err(e) => {
                        warn!("Redis GET failed for {key}: {e}");
                        None
                    }
                }
            })
        })
    }

    /// Store transport query result in Redis with TTL.
    pub fn set_transport(
        &self,
        transport_type: &str,
        from: &str,
        to: &str,
        date: &str,
        value: &str,
    ) {
        let Some(client) = self.client.as_ref() else {
            return;
        };
        let key = Self::transport_key(transport_type, from, to, date);
        let ttl = self.ttl_seconds;
        let value = value.to_string();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let mut conn = match client.get_multiplexed_async_connection().await {
                    Ok(c) => c,
                    Err(e) => {
                        warn!("Redis SET connection failed: {e}");
                        return;
                    }
                };
                match redis::AsyncCommands::set_ex::<_, _, ()>(&mut conn, &key, &value, ttl).await
                {
                    Ok(()) => info!("Redis cache SET: {key} (TTL={ttl}s)"),
                    Err(e) => warn!("Redis SET failed for {key}: {e}"),
                }
            });
        });
    }
}
