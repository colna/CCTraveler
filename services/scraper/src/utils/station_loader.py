"""Download and import full 12306 station codes (~3000 stations)."""
from __future__ import annotations

import json
import logging
import re
import sqlite3
from pathlib import Path
from typing import List, Dict

import httpx

from .geo_lookup import db_path

logger = logging.getLogger(__name__)

STATION_NAME_JS_URL = "https://kyfw.12306.cn/otn/resources/js/framework/station_name.js"


def parse_station_name_js(js_text: str) -> List[Dict[str, str]]:
    """Parse the 12306 station_name.js response.

    Format: var station_names='@bjb|北京北|VAP|beijingbei|bjb|0@bjd|北京东|BOP|beijingdong|bjd|1@...'
    Each entry: @pinyin_abbr|station_name|station_code|pinyin_full|pinyin_short|index
    """
    match = re.search(r"'(.+)'", js_text)
    if not match:
        raise ValueError("Cannot parse station_name.js: no quoted string found")

    raw = match.group(1)
    entries = raw.split("@")
    stations: List[Dict[str, str]] = []

    for entry in entries:
        if not entry.strip():
            continue
        parts = entry.split("|")
        if len(parts) < 4:
            continue

        station_name = parts[1]
        station_code = parts[2]
        pinyin = parts[3]

        # Derive city name: remove directional/area suffixes
        # 12306 names: "北京北", "上海虹桥", "成都东" (no "站" suffix)
        city = station_name
        for suffix in ("东站", "西站", "南站", "北站", "站", "虹桥", "大兴", "朝阳", "通州", "丰台", "东", "西", "南", "北"):
            if city.endswith(suffix) and len(city) - len(suffix) >= 2:
                city = city[: -len(suffix)]
                break

        stations.append({
            "city": city,
            "station_name": station_name,
            "station_code": station_code,
            "pinyin": pinyin,
        })

    return stations


async def download_station_codes() -> List[Dict[str, str]]:
    """Download station codes from 12306."""
    logger.info("Downloading station codes from 12306...")
    async with httpx.AsyncClient(timeout=30.0) as client:
        resp = await client.get(STATION_NAME_JS_URL)
        resp.raise_for_status()

    stations = parse_station_name_js(resp.text)
    logger.info("Parsed %d stations from 12306", len(stations))
    return stations


def import_stations_to_db(stations: List[Dict[str, str]], db: Path | None = None) -> int:
    """Write station codes into the SQLite database.

    Creates the station_codes table if it does not exist.
    Returns the number of stations imported.
    """
    db = db or db_path()
    conn = sqlite3.connect(db)
    try:
        conn.execute("""
            CREATE TABLE IF NOT EXISTS station_codes (
                city TEXT NOT NULL,
                station_name TEXT NOT NULL,
                station_code TEXT NOT NULL PRIMARY KEY,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )
        """)
        conn.execute("CREATE INDEX IF NOT EXISTS idx_station_codes_city ON station_codes(city)")
        conn.execute("CREATE INDEX IF NOT EXISTS idx_station_codes_name ON station_codes(station_name)")

        conn.execute("DELETE FROM station_codes")

        for s in stations:
            conn.execute(
                "INSERT OR REPLACE INTO station_codes (city, station_name, station_code) VALUES (?, ?, ?)",
                (s["city"], s["station_name"], s["station_code"]),
            )

        conn.commit()
        count = conn.execute("SELECT COUNT(*) FROM station_codes").fetchone()[0]
        logger.info("Imported %d station codes into DB", count)
        return count
    finally:
        conn.close()


def save_stations_json(stations: List[Dict[str, str]], output: Path) -> None:
    """Save station codes to a JSON file (without pinyin, to match existing format)."""
    data = [
        {"city": s["city"], "station_name": s["station_name"], "station_code": s["station_code"]}
        for s in stations
    ]
    output.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    logger.info("Saved %d stations to %s", len(data), output)


async def ensure_stations_loaded() -> int:
    """Ensure station codes are loaded in the DB. Downloads if needed.

    Returns the number of stations in the DB.
    """
    db = db_path()
    conn = sqlite3.connect(db)
    try:
        conn.execute("""
            CREATE TABLE IF NOT EXISTS station_codes (
                city TEXT NOT NULL,
                station_name TEXT NOT NULL,
                station_code TEXT NOT NULL PRIMARY KEY,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )
        """)
        count = conn.execute("SELECT COUNT(*) FROM station_codes").fetchone()[0]
    finally:
        conn.close()

    if count >= 100:
        return count

    logger.info("Only %d stations in DB, downloading full dataset...", count)
    stations = await download_station_codes()
    return import_stations_to_db(stations, db)


if __name__ == "__main__":
    import asyncio

    logging.basicConfig(level=logging.INFO)

    async def main():
        stations = await download_station_codes()
        print(f"Downloaded {len(stations)} stations")

        project_root = Path(__file__).resolve().parents[4]
        json_path = project_root / "data" / "geo" / "stations.json"
        save_stations_json(stations, json_path)

        count = import_stations_to_db(stations)
        print(f"Imported {count} stations into DB")

    asyncio.run(main())
