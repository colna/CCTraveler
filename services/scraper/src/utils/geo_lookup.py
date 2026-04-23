"""Shared geography lookups backed by the SQLite database."""
from __future__ import annotations

import sqlite3
from pathlib import Path
from typing import Dict, List, Optional


def _project_root() -> Path:
    return Path(__file__).resolve().parents[4]


def db_path() -> Path:
    return _project_root() / "data" / "cctraveler.db"


def _connect() -> sqlite3.Connection:
    conn = sqlite3.connect(db_path())
    conn.row_factory = sqlite3.Row
    return conn


def _normalize_city_name(name: str) -> str:
    normalized = name.strip()
    for start, end in (("(", ")"), ("（", "）")):
        if normalized.endswith(end) and start in normalized:
            normalized = normalized[: normalized.rfind(start)].strip()
    return normalized


def resolve_city(city: str) -> Optional[Dict[str, str]]:
    trimmed = city.strip()
    if not trimmed:
        return None

    normalized = _normalize_city_name(trimmed)
    conn = _connect()
    try:
        row = conn.execute(
            """
            SELECT DISTINCT c.id, c.name, m.source_id, m.pinyin
            FROM cities c
            LEFT JOIN city_mappings m ON m.city_id = c.id AND m.source = 'ctrip'
            WHERE c.name = ?
               OR c.name = ?
               OR lower(COALESCE(c.name_en, '')) = lower(?)
               OR lower(COALESCE(c.name_en, '')) = lower(?)
               OR m.source_name = ?
               OR m.source_name = ?
               OR lower(COALESCE(m.pinyin, '')) = lower(?)
               OR lower(COALESCE(m.pinyin, '')) = lower(?)
               OR m.source_id = ?
            ORDER BY CASE
               WHEN c.name = ? THEN 0
               WHEN c.name = ? THEN 1
               WHEN m.source_name = ? THEN 2
               WHEN m.source_name = ? THEN 3
               WHEN lower(COALESCE(m.pinyin, '')) = lower(?) THEN 4
               WHEN lower(COALESCE(m.pinyin, '')) = lower(?) THEN 5
               WHEN m.source_id = ? THEN 6
               ELSE 7
            END
            LIMIT 1
            """,
            (
                trimmed,
                normalized,
                trimmed,
                normalized,
                trimmed,
                normalized,
                trimmed,
                normalized,
                trimmed,
                trimmed,
                normalized,
                trimmed,
                normalized,
                trimmed,
                normalized,
                trimmed,
            ),
        ).fetchone()
        if row is None:
            return None
        return {
            "id": row["id"],
            "name": row["name"],
            "source_id": row["source_id"] or row["id"],
            "pinyin": row["pinyin"] or "",
        }
    finally:
        conn.close()


def resolve_city_id(city: str) -> str:
    if city.isdigit():
        return city
    resolved = resolve_city(city)
    return resolved["source_id"] if resolved else city


def list_supported_cities(query: Optional[str] = None) -> List[Dict[str, str]]:
    conn = _connect()
    try:
        rows = conn.execute(
            """
            SELECT c.id, c.name, COALESCE(m.pinyin, '') AS pinyin, COALESCE(m.source_id, c.id) AS source_id
            FROM cities c
            LEFT JOIN city_mappings m ON m.city_id = c.id AND m.source = 'ctrip'
            ORDER BY c.name ASC
            """
        ).fetchall()
    finally:
        conn.close()

    cities = [
        {
            "id": row["source_id"],
            "city_id": row["id"],
            "name": row["name"],
            "pinyin": row["pinyin"],
        }
        for row in rows
    ]
    if query:
        query_lower = query.lower()
        cities = [
            city for city in cities
            if query_lower in city["name"].lower() or query_lower in city["pinyin"].lower()
        ]
    return cities


def get_station_code(city_or_station: str) -> Optional[str]:
    resolved = resolve_city(city_or_station)
    candidate_names = [city_or_station.strip()]
    if resolved:
        candidate_names.append(resolved["name"])
    conn = _connect()
    try:
        for candidate in dict.fromkeys(name for name in candidate_names if name):
            row = conn.execute(
                """
                SELECT station_code
                FROM station_codes
                WHERE city = ? OR station_name = ?
                ORDER BY CASE WHEN station_name = ? THEN 0 ELSE 1 END, station_name ASC
                LIMIT 1
                """,
                (candidate, candidate, candidate),
            ).fetchone()
            if row is not None:
                return row["station_code"]
        return None
    finally:
        conn.close()


def get_airport_code(city: str) -> str:
    resolved = resolve_city(city)
    candidate_names = [city.strip()]
    if resolved:
        candidate_names.append(resolved["name"])
    conn = _connect()
    try:
        for candidate in dict.fromkeys(name for name in candidate_names if name):
            row = conn.execute(
                """
                SELECT airport_code
                FROM airport_codes
                WHERE city = ?
                ORDER BY airport_name ASC
                LIMIT 1
                """,
                (candidate,),
            ).fetchone()
            if row is not None:
                return row["airport_code"]
        return "UNK"
    finally:
        conn.close()
