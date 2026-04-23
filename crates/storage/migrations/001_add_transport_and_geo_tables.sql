-- Migration: Add transport and geography tables for v0.2
-- Date: 2026-04-23

-- ============================================================
-- 火车票相关表
-- ============================================================

CREATE TABLE IF NOT EXISTS trains (
    id TEXT PRIMARY KEY,              -- 车次号 (如 G1234)
    train_type TEXT NOT NULL,         -- 车型 (G/D/C/K/T/Z)
    from_station TEXT NOT NULL,       -- 出发站
    to_station TEXT NOT NULL,         -- 到达站
    from_city TEXT NOT NULL,          -- 出发城市
    to_city TEXT NOT NULL,            -- 到达城市
    depart_time TEXT NOT NULL,        -- 出发时间 (HH:MM)
    arrive_time TEXT NOT NULL,        -- 到达时间 (HH:MM)
    duration_minutes INTEGER NOT NULL,-- 运行时长（分钟）
    distance_km INTEGER,              -- 里程（公里）
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_trains_route ON trains(from_city, to_city);
CREATE INDEX IF NOT EXISTS idx_trains_type ON trains(train_type);

CREATE TABLE IF NOT EXISTS train_prices (
    id TEXT PRIMARY KEY,
    train_id TEXT NOT NULL REFERENCES trains(id),
    seat_type TEXT NOT NULL,          -- 座位类型 (商务座/一等座/二等座/硬卧/软卧)
    price REAL NOT NULL,              -- 价格（元）
    available_seats INTEGER,          -- 余票数量 (-1 表示未知)
    travel_date TEXT NOT NULL,        -- 乘车日期 (YYYY-MM-DD)
    scraped_at TEXT NOT NULL DEFAULT (datetime('now')),
    source TEXT NOT NULL DEFAULT '12306'
);

CREATE INDEX IF NOT EXISTS idx_train_prices_date ON train_prices(travel_date);
CREATE INDEX IF NOT EXISTS idx_train_prices_train ON train_prices(train_id);

-- ============================================================
-- 机票相关表
-- ============================================================

CREATE TABLE IF NOT EXISTS flights (
    id TEXT PRIMARY KEY,              -- 航班号 (如 CA1234)
    airline TEXT NOT NULL,            -- 航空公司
    from_airport TEXT NOT NULL,       -- 出发机场代码 (如 PEK)
    to_airport TEXT NOT NULL,         -- 到达机场代码 (如 SHA)
    from_city TEXT NOT NULL,          -- 出发城市
    to_city TEXT NOT NULL,            -- 到达城市
    depart_time TEXT NOT NULL,        -- 出发时间 (HH:MM)
    arrive_time TEXT NOT NULL,        -- 到达时间 (HH:MM)
    duration_minutes INTEGER NOT NULL,-- 飞行时长（分钟）
    aircraft_type TEXT,               -- 机型 (如 A320)
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_flights_route ON flights(from_city, to_city);
CREATE INDEX IF NOT EXISTS idx_flights_airline ON flights(airline);

CREATE TABLE IF NOT EXISTS flight_prices (
    id TEXT PRIMARY KEY,
    flight_id TEXT NOT NULL REFERENCES flights(id),
    cabin_class TEXT NOT NULL,        -- 舱位等级 (头等舱/商务舱/经济舱)
    price REAL NOT NULL,              -- 价格（元）
    discount REAL,                    -- 折扣 (如 0.8 表示 8 折)
    available_seats INTEGER,          -- 余票数量
    travel_date TEXT NOT NULL,        -- 出行日期 (YYYY-MM-DD)
    scraped_at TEXT NOT NULL DEFAULT (datetime('now')),
    source TEXT NOT NULL              -- 数据来源 (ctrip/qunar/fliggy)
);

CREATE INDEX IF NOT EXISTS idx_flight_prices_date ON flight_prices(travel_date);
CREATE INDEX IF NOT EXISTS idx_flight_prices_flight ON flight_prices(flight_id);

-- ============================================================
-- 城市地理信息表
-- ============================================================

CREATE TABLE IF NOT EXISTS cities (
    id TEXT PRIMARY KEY,              -- 城市 ID
    name TEXT NOT NULL,               -- 城市名称
    name_en TEXT,                     -- 英文名
    province TEXT NOT NULL,           -- 省份
    latitude REAL,                    -- 纬度
    longitude REAL,                   -- 经度
    population INTEGER,               -- 人口
    area_km2 REAL,                    -- 面积（平方公里）
    tier INTEGER,                     -- 城市等级 (1/2/3/4/5)
    description TEXT,                 -- 城市简介
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_cities_name ON cities(name);
CREATE INDEX IF NOT EXISTS idx_cities_province ON cities(province);

CREATE TABLE IF NOT EXISTS districts (
    id TEXT PRIMARY KEY,
    city_id TEXT NOT NULL REFERENCES cities(id),
    name TEXT NOT NULL,               -- 区域名称 (如 "汇川区")
    name_en TEXT,
    latitude REAL,
    longitude REAL,
    description TEXT,                 -- 区域特点
    tags TEXT                         -- 标签 (JSON 数组: ["商业区","交通枢纽"])
);

CREATE INDEX IF NOT EXISTS idx_districts_city ON districts(city_id);

CREATE TABLE IF NOT EXISTS attractions (
    id TEXT PRIMARY KEY,
    city_id TEXT NOT NULL REFERENCES cities(id),
    district_id TEXT REFERENCES districts(id),
    name TEXT NOT NULL,               -- 景点名称
    name_en TEXT,
    category TEXT NOT NULL,           -- 类别 (历史/自然/娱乐/购物)
    rating REAL,                      -- 评分 (0-5)
    latitude REAL,
    longitude REAL,
    address TEXT,
    description TEXT,
    opening_hours TEXT,               -- 开放时间
    ticket_price REAL,                -- 门票价格
    visit_duration_hours REAL         -- 建议游玩时长（小时）
);

CREATE INDEX IF NOT EXISTS idx_attractions_city ON attractions(city_id);
CREATE INDEX IF NOT EXISTS idx_attractions_category ON attractions(category);

-- ============================================================
-- 知识维基表
-- ============================================================

CREATE TABLE IF NOT EXISTS wiki_entries (
    id TEXT PRIMARY KEY,
    topic TEXT NOT NULL,              -- 主题 (user_history/city_guide/route_tips)
    key TEXT NOT NULL,                -- 键 (如 "user_budget_range")
    value TEXT NOT NULL,              -- 值 (JSON 格式)
    metadata TEXT,                    -- 元数据 (JSON)
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_wiki_topic ON wiki_entries(topic);
CREATE INDEX IF NOT EXISTS idx_wiki_key ON wiki_entries(key);
CREATE UNIQUE INDEX IF NOT EXISTS idx_wiki_topic_key ON wiki_entries(topic, key);
