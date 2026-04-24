-- Migration: Add price subscriptions table for v0.3 price monitoring
-- Date: 2026-04-24

CREATE TABLE IF NOT EXISTS price_subscriptions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    from_city TEXT NOT NULL,
    to_city TEXT NOT NULL,
    transport_type TEXT NOT NULL,     -- 'train' or 'flight'
    threshold REAL NOT NULL,          -- price threshold for notification
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_price_subs_user ON price_subscriptions(user_id);
CREATE INDEX IF NOT EXISTS idx_price_subs_active ON price_subscriptions(is_active);
CREATE INDEX IF NOT EXISTS idx_price_subs_route ON price_subscriptions(from_city, to_city);
