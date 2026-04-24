-- Migration: Add performance indexes for v0.3
-- Date: 2026-04-24

-- Composite indexes for price queries (date + scrape time)
CREATE INDEX IF NOT EXISTS idx_train_prices_date_scraped ON train_prices(travel_date, scraped_at);
CREATE INDEX IF NOT EXISTS idx_flight_prices_date_scraped ON flight_prices(travel_date, scraped_at);

-- Composite indexes for route + date queries
CREATE INDEX IF NOT EXISTS idx_trains_route_type ON trains(from_city, to_city, train_type);
CREATE INDEX IF NOT EXISTS idx_flights_route_depart ON flights(from_city, to_city, depart_time);

-- Price subscription indexes
CREATE INDEX IF NOT EXISTS idx_price_subs_active ON price_subscriptions(is_active, user_id);
CREATE INDEX IF NOT EXISTS idx_price_subs_route ON price_subscriptions(from_city, to_city, transport_type);

-- City geo indexes
CREATE INDEX IF NOT EXISTS idx_cities_coords ON cities(latitude, longitude);
CREATE INDEX IF NOT EXISTS idx_attractions_city_cat ON attractions(city_id, category);

-- Hotel price performance
CREATE INDEX IF NOT EXISTS idx_price_snapshots_room ON price_snapshots(room_id, scraped_at);
