"""城市地理数据导入工具"""
import json
import sqlite3
from pathlib import Path
from typing import Any, Dict, Iterable, List, Optional


def connect_db(db_path: str) -> sqlite3.Connection:
    conn = sqlite3.connect(db_path)
    conn.execute("PRAGMA foreign_keys=ON")
    return conn


def init_schema(conn: sqlite3.Connection):
    migrations_dir = Path(__file__).parent.parent / "crates" / "storage" / "migrations"
    for name in ["001_add_transport_and_geo_tables.sql", "002_add_geo_lookup_tables.sql"]:
        migration_path = migrations_dir / name
        if migration_path.exists():
            conn.executescript(migration_path.read_text(encoding='utf-8'))


def load_json(path: Path):
    with open(path, 'r', encoding='utf-8') as f:
        return json.load(f)


def normalize_city_name(name: str) -> str:
    normalized = (name or '').strip()
    for start, end in [('(', ')'), ('（', '）')]:
        if normalized.endswith(end) and start in normalized:
            normalized = normalized[: normalized.rfind(start)].strip()
    return normalized


def ensure_city_record(conn: sqlite3.Connection, city_id: str, name: str, name_en: Optional[str] = None):
    conn.execute(
        """
        INSERT INTO cities (
            id, name, name_en, province, latitude, longitude, population, area_km2, tier, description
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            name_en = COALESCE(cities.name_en, excluded.name_en)
        """,
        (city_id, name, name_en, '', None, None, None, None, None, None),
    )


def load_existing_city_ids(conn: sqlite3.Connection) -> Dict[str, str]:
    rows = conn.execute("SELECT id, name FROM cities").fetchall()
    return {row[1]: row[0] for row in rows}


def import_full_city_catalog(conn: sqlite3.Connection, ctrip_cities_file: Path):
    payload = load_json(ctrip_cities_file)
    cities = payload.get('cities', [])
    existing = load_existing_city_ids(conn)

    for city in cities:
        source_id = str(city['id'])
        name = city['name']
        pinyin = city.get('pinyin')
        canonical_id = existing.get(name) or f"ctrip:{source_id}"
        ensure_city_record(conn, canonical_id, name)
        conn.execute(
            """
            INSERT INTO city_mappings (city_id, source, source_id, source_name, pinyin)
            VALUES (?, 'ctrip', ?, ?, ?)
            ON CONFLICT(source, source_id) DO UPDATE SET
                city_id = excluded.city_id,
                source_name = excluded.source_name,
                pinyin = excluded.pinyin
            """,
            (canonical_id, source_id, name, pinyin),
        )

    count = conn.execute("SELECT COUNT(*) FROM city_mappings WHERE source = 'ctrip'").fetchone()[0]
    print(f"✅ 导入 {count} 个完整城市映射")


def import_cities(conn: sqlite3.Connection, cities_file: Path):
    cities = load_json(cities_file)

    for city in cities:
        conn.execute(
            """
            INSERT INTO cities (
                id, name, name_en, province, tier, population,
                area_km2, latitude, longitude, description
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                name_en = excluded.name_en,
                province = excluded.province,
                tier = excluded.tier,
                population = excluded.population,
                area_km2 = excluded.area_km2,
                latitude = excluded.latitude,
                longitude = excluded.longitude,
                description = excluded.description
            """,
            (
                city.get('id'),
                city.get('name'),
                city.get('name_en'),
                city.get('province') or '',
                city.get('tier'),
                city.get('population'),
                city.get('area_km2'),
                city.get('latitude'),
                city.get('longitude'),
                city.get('description'),
            ),
        )
        conn.execute(
            """
            INSERT OR IGNORE INTO city_mappings (city_id, source, source_id, source_name, pinyin)
            VALUES (?, 'geo', ?, ?, ?)
            """,
            (
                city.get('id'),
                city.get('id'),
                city.get('name'),
                (city.get('name_en') or '').lower() or None,
            ),
        )

    count = conn.execute("SELECT COUNT(*) FROM cities").fetchone()[0]
    print(f"✅ 导入 {count} 个城市")


def import_districts(conn: sqlite3.Connection, districts_file: Path):
    districts = load_json(districts_file)

    for district in districts:
        conn.execute(
            """
            INSERT OR REPLACE INTO districts (
                id, city_id, name, description, tags
            ) VALUES (?, ?, ?, ?, ?)
            """,
            (
                district.get('id'),
                district.get('city_id'),
                district.get('name'),
                district.get('description'),
                json.dumps(district.get('tags', []), ensure_ascii=False),
            ),
        )

    count = conn.execute("SELECT COUNT(*) FROM districts").fetchone()[0]
    print(f"✅ 导入 {count} 个区域")


def import_attractions(conn: sqlite3.Connection, attractions_file: Path):
    attractions = load_json(attractions_file)

    for attraction in attractions:
        conn.execute(
            """
            INSERT OR REPLACE INTO attractions (
                id, city_id, name, category, rating, ticket_price,
                visit_duration_hours, latitude, longitude, description
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                attraction.get('id'),
                attraction.get('city_id'),
                attraction.get('name'),
                attraction.get('category'),
                attraction.get('rating'),
                attraction.get('ticket_price'),
                attraction.get('visit_duration_hours'),
                attraction.get('latitude'),
                attraction.get('longitude'),
                attraction.get('description'),
            ),
        )

    count = conn.execute("SELECT COUNT(*) FROM attractions").fetchone()[0]
    print(f"✅ 导入 {count} 个景点")


def resolve_city_id(conn: sqlite3.Connection, city_name: str) -> Optional[str]:
    normalized = normalize_city_name(city_name)
    row = conn.execute(
        """
        SELECT c.id
        FROM cities c
        LEFT JOIN city_mappings m ON m.city_id = c.id
        WHERE c.name = ? OR c.name = ? OR m.source_name = ? OR m.source_name = ? OR lower(COALESCE(m.pinyin, '')) = lower(?)
        ORDER BY CASE
            WHEN c.name = ? THEN 0
            WHEN c.name = ? THEN 1
            WHEN m.source_name = ? THEN 2
            WHEN m.source_name = ? THEN 3
            ELSE 4
        END
        LIMIT 1
        """,
        (
            city_name,
            normalized,
            city_name,
            normalized,
            city_name,
            city_name,
            normalized,
            city_name,
            normalized,
        ),
    ).fetchone()
    return row[0] if row else None


def import_station_codes(conn: sqlite3.Connection, stations_file: Path):
    stations = load_json(stations_file)

    for station in stations:
        city = normalize_city_name(station.get('city', ''))
        city_id = resolve_city_id(conn, city)
        if not city_id:
            continue
        conn.execute(
            """
            INSERT OR REPLACE INTO station_codes (city, station_name, station_code)
            VALUES (?, ?, ?)
            """,
            (
                city,
                station.get('station_name'),
                station.get('station_code'),
            ),
        )

    count = conn.execute("SELECT COUNT(*) FROM station_codes").fetchone()[0]
    print(f"✅ 导入 {count} 个车站代码")


def import_airport_codes(conn: sqlite3.Connection, airports_file: Path):
    airports = load_json(airports_file)

    for airport in airports:
        city = normalize_city_name(airport.get('city', ''))
        city_id = resolve_city_id(conn, city)
        if not city_id:
            continue
        conn.execute(
            """
            INSERT OR REPLACE INTO airport_codes (
                city, airport_name, airport_code, iata_code, icao_code
            ) VALUES (?, ?, ?, ?, ?)
            """,
            (
                city,
                airport.get('airport_name'),
                airport.get('airport_code'),
                airport.get('iata_code'),
                airport.get('icao_code'),
            ),
        )

    count = conn.execute("SELECT COUNT(*) FROM airport_codes").fetchone()[0]
    print(f"✅ 导入 {count} 个机场代码")


if __name__ == "__main__":
    import sys

    if len(sys.argv) < 2:
        print("用法: python import_geo_data.py <db_path>")
        sys.exit(1)

    db_path = sys.argv[1]
    data_dir = Path(__file__).parent.parent / "data"
    geo_dir = data_dir / "geo"

    print("开始导入地理数据...")

    conn = connect_db(db_path)
    try:
        init_schema(conn)

        cities_file = geo_dir / "cities.json"
        if cities_file.exists():
            import_cities(conn, cities_file)
        else:
            print(f"⚠️  未找到城市数据文件: {cities_file}")

        full_cities_file = data_dir / "ctrip_cities.json"
        if full_cities_file.exists():
            import_full_city_catalog(conn, full_cities_file)
        else:
            print(f"⚠️  未找到完整城市数据文件: {full_cities_file}")

        districts_file = geo_dir / "districts.json"
        if districts_file.exists():
            import_districts(conn, districts_file)
        else:
            print(f"⚠️  未找到区域数据文件: {districts_file}")

        attractions_file = geo_dir / "attractions.json"
        if attractions_file.exists():
            import_attractions(conn, attractions_file)
        else:
            print(f"⚠️  未找到景点数据文件: {attractions_file}")

        stations_file = geo_dir / "stations.json"
        if stations_file.exists():
            import_station_codes(conn, stations_file)
        else:
            print(f"⚠️  未找到车站代码文件: {stations_file}")

        airports_file = geo_dir / "airports.json"
        if airports_file.exists():
            import_airport_codes(conn, airports_file)
        else:
            print(f"⚠️  未找到机场代码文件: {airports_file}")

        conn.commit()
    finally:
        conn.close()

    print("\n✅ 地理数据导入完成！")
