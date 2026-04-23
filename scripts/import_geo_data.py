"""城市地理数据导入工具"""
import json
import sqlite3
from pathlib import Path
from typing import List, Dict, Any

def import_cities(db_path: str, cities_file: str):
    """导入城市数据"""
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    with open(cities_file, 'r', encoding='utf-8') as f:
        cities = json.load(f)

    for city in cities:
        cursor.execute("""
            INSERT OR REPLACE INTO cities (
                id, name, name_en, province, tier, population,
                area_km2, latitude, longitude, description
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """, (
            city.get('id'),
            city.get('name'),
            city.get('name_en'),
            city.get('province'),
            city.get('tier'),
            city.get('population'),
            city.get('area_km2'),
            city.get('latitude'),
            city.get('longitude'),
            city.get('description'),
        ))

    conn.commit()
    count = cursor.execute("SELECT COUNT(*) FROM cities").fetchone()[0]
    print(f"✅ 导入 {count} 个城市")
    conn.close()

def import_districts(db_path: str, districts_file: str):
    """导入区域数据"""
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    with open(districts_file, 'r', encoding='utf-8') as f:
        districts = json.load(f)

    for district in districts:
        cursor.execute("""
            INSERT OR REPLACE INTO districts (
                id, city_id, name, description, tags
            ) VALUES (?, ?, ?, ?, ?)
        """, (
            district.get('id'),
            district.get('city_id'),
            district.get('name'),
            district.get('description'),
            json.dumps(district.get('tags', []), ensure_ascii=False),
        ))

    conn.commit()
    count = cursor.execute("SELECT COUNT(*) FROM districts").fetchone()[0]
    print(f"✅ 导入 {count} 个区域")
    conn.close()

def import_attractions(db_path: str, attractions_file: str):
    """导入景点数据"""
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    with open(attractions_file, 'r', encoding='utf-8') as f:
        attractions = json.load(f)

    for attraction in attractions:
        cursor.execute("""
            INSERT OR REPLACE INTO attractions (
                id, city_id, name, category, rating, ticket_price,
                visit_duration_hours, latitude, longitude, description
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """, (
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
        ))

    conn.commit()
    count = cursor.execute("SELECT COUNT(*) FROM attractions").fetchone()[0]
    print(f"✅ 导入 {count} 个景点")
    conn.close()

def import_station_codes(db_path: str, stations_file: str):
    """导入车站代码映射"""
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    # 创建临时表存储车站代码
    cursor.execute("""
        CREATE TABLE IF NOT EXISTS station_codes (
            city TEXT NOT NULL,
            station_name TEXT NOT NULL,
            station_code TEXT NOT NULL PRIMARY KEY,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        )
    """)

    with open(stations_file, 'r', encoding='utf-8') as f:
        stations = json.load(f)

    for station in stations:
        cursor.execute("""
            INSERT OR REPLACE INTO station_codes (city, station_name, station_code)
            VALUES (?, ?, ?)
        """, (
            station.get('city'),
            station.get('station_name'),
            station.get('station_code'),
        ))

    conn.commit()
    count = cursor.execute("SELECT COUNT(*) FROM station_codes").fetchone()[0]
    print(f"✅ 导入 {count} 个车站代码")
    conn.close()

def import_airport_codes(db_path: str, airports_file: str):
    """导入机场代码映射"""
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    # 创建临时表存储机场代码
    cursor.execute("""
        CREATE TABLE IF NOT EXISTS airport_codes (
            city TEXT NOT NULL,
            airport_name TEXT NOT NULL,
            airport_code TEXT NOT NULL PRIMARY KEY,
            iata_code TEXT,
            icao_code TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        )
    """)

    with open(airports_file, 'r', encoding='utf-8') as f:
        airports = json.load(f)

    for airport in airports:
        cursor.execute("""
            INSERT OR REPLACE INTO airport_codes (
                city, airport_name, airport_code, iata_code, icao_code
            ) VALUES (?, ?, ?, ?, ?)
        """, (
            airport.get('city'),
            airport.get('airport_name'),
            airport.get('airport_code'),
            airport.get('iata_code'),
            airport.get('icao_code'),
        ))

    conn.commit()
    count = cursor.execute("SELECT COUNT(*) FROM airport_codes").fetchone()[0]
    print(f"✅ 导入 {count} 个机场代码")
    conn.close()

if __name__ == "__main__":
    import sys

    if len(sys.argv) < 2:
        print("用法: python import_geo_data.py <db_path>")
        sys.exit(1)

    db_path = sys.argv[1]
    data_dir = Path(__file__).parent.parent / "data" / "geo"

    print("开始导入地理数据...")

    # 导入城市数据
    cities_file = data_dir / "cities.json"
    if cities_file.exists():
        import_cities(db_path, str(cities_file))
    else:
        print(f"⚠️  未找到城市数据文件: {cities_file}")

    # 导入区域数据
    districts_file = data_dir / "districts.json"
    if districts_file.exists():
        import_districts(db_path, str(districts_file))
    else:
        print(f"⚠️  未找到区域数据文件: {districts_file}")

    # 导入景点数据
    attractions_file = data_dir / "attractions.json"
    if attractions_file.exists():
        import_attractions(db_path, str(attractions_file))
    else:
        print(f"⚠️  未找到景点数据文件: {attractions_file}")

    # 导入车站代码
    stations_file = data_dir / "stations.json"
    if stations_file.exists():
        import_station_codes(db_path, str(stations_file))
    else:
        print(f"⚠️  未找到车站代码文件: {stations_file}")

    # 导入机场代码
    airports_file = data_dir / "airports.json"
    if airports_file.exists():
        import_airport_codes(db_path, str(airports_file))
    else:
        print(f"⚠️  未找到机场代码文件: {airports_file}")

    print("\n✅ 地理数据导入完成！")
