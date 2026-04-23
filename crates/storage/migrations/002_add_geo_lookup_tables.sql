-- Migration: Add geo lookup tables for v0.3
-- Date: 2026-04-23

CREATE TABLE IF NOT EXISTS city_mappings (
    city_id TEXT NOT NULL REFERENCES cities(id),
    source TEXT NOT NULL,
    source_id TEXT NOT NULL,
    source_name TEXT NOT NULL,
    pinyin TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (source, source_id)
);

CREATE INDEX IF NOT EXISTS idx_city_mappings_city_id ON city_mappings(city_id);
CREATE INDEX IF NOT EXISTS idx_city_mappings_source_name ON city_mappings(source_name);
CREATE INDEX IF NOT EXISTS idx_city_mappings_pinyin ON city_mappings(pinyin);

CREATE TABLE IF NOT EXISTS station_codes (
    city TEXT NOT NULL,
    station_name TEXT NOT NULL,
    station_code TEXT NOT NULL PRIMARY KEY,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_station_codes_city ON station_codes(city);
CREATE INDEX IF NOT EXISTS idx_station_codes_name ON station_codes(station_name);

CREATE TABLE IF NOT EXISTS airport_codes (
    city TEXT NOT NULL,
    airport_name TEXT NOT NULL,
    airport_code TEXT NOT NULL PRIMARY KEY,
    iata_code TEXT,
    icao_code TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_airport_codes_city ON airport_codes(city);
CREATE INDEX IF NOT EXISTS idx_airport_codes_name ON airport_codes(airport_name);
