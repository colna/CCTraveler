"""12306 train ticket fetcher using JSON API (no browser needed)."""
from __future__ import annotations

import logging
from datetime import datetime
from typing import Dict, List, Optional

import httpx

from ..utils.geo_lookup import get_station_code
from ..utils.station_loader import ensure_stations_loaded
from .types import ScrapedTrain, TrainSeatPrice

logger = logging.getLogger(__name__)

INIT_URL = "https://kyfw.12306.cn/otn/leftTicket/init?linktypeid=dc"
QUERY_URL = "https://kyfw.12306.cn/otn/leftTicket/queryZ"

BROWSER_HEADERS = {
    "User-Agent": (
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
        "AppleWebKit/537.36 (KHTML, like Gecko) "
        "Chrome/125.0.0.0 Safari/537.36"
    ),
    "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
    "Accept-Language": "zh-CN,zh;q=0.9,en;q=0.8",
}

API_HEADERS = {
    "Accept": "application/json, text/javascript, */*; q=0.01",
    "Referer": "https://kyfw.12306.cn/otn/leftTicket/init",
    "X-Requested-With": "XMLHttpRequest",
}

# 12306 pipe-delimited field indices (0-based)
# Format: secretStr|...|train_no|train_id|from_code|to_code|start_code|end_code|
#         depart_time|arrive_time|duration|...|seats...
_IDX_TRAIN_NO = 3        # 车次号 e.g. G1234
_IDX_FROM_CODE = 6       # 出发站电报码
_IDX_TO_CODE = 7         # 到达站电报码
_IDX_DEPART = 8          # 出发时间
_IDX_ARRIVE = 9          # 到达时间
_IDX_DURATION = 10       # 历时 e.g. "04:30"

# Seat availability field indices
_SEAT_FIELDS = {
    "商务座": 32,  # or 特等座
    "一等座": 31,
    "二等座": 30,
    "高级软卧": 21,
    "软卧": 23,
    "动卧": 33,
    "硬卧": 28,
    "软座": 24,
    "硬座": 29,
    "无座": 26,
}


def parse_train_type(train_id: str) -> str:
    prefix = train_id[0] if train_id else ""
    return prefix if prefix in ("G", "D", "C", "K", "T", "Z") else "其他"


def parse_duration(duration_str: str) -> int:
    """Parse "HH:MM" duration to minutes."""
    if ":" in duration_str:
        parts = duration_str.split(":")
        try:
            return int(parts[0]) * 60 + int(parts[1])
        except ValueError:
            return 0
    return 0


def _parse_available_seats(value: str) -> Optional[int]:
    """Parse seat availability string from 12306.

    Returns:
        int: number of available seats
        -1: available but count unknown ("有")
        None: not available or no data ("无", "", "--")
    """
    value = value.strip()
    if not value or value in ("", "--", "*"):
        return None
    if value == "无":
        return None
    if value == "有":
        return -1
    try:
        return int(value)
    except ValueError:
        return None


def _parse_train_result(
    raw: str,
    station_map: Dict[str, str],
    from_city: str,
    to_city: str,
) -> Optional[ScrapedTrain]:
    """Parse a single pipe-delimited train result string."""
    fields = raw.split("|")
    if len(fields) < 34:
        return None

    train_id = fields[_IDX_TRAIN_NO]
    if not train_id:
        return None

    from_code = fields[_IDX_FROM_CODE]
    to_code = fields[_IDX_TO_CODE]
    depart_time = fields[_IDX_DEPART]
    arrive_time = fields[_IDX_ARRIVE]
    duration_str = fields[_IDX_DURATION]

    # Resolve station names from the map
    from_station = station_map.get(from_code, from_code)
    to_station = station_map.get(to_code, to_code)

    # Parse seat availability
    seats: List[TrainSeatPrice] = []
    for seat_type, idx in _SEAT_FIELDS.items():
        if idx >= len(fields):
            continue
        available = _parse_available_seats(fields[idx])
        if available is not None:
            seats.append(TrainSeatPrice(
                seat_type=seat_type,
                price=0.0,  # 12306 queryZ doesn't include prices
                available_seats=available,
            ))

    return ScrapedTrain(
        train_id=train_id,
        train_type=parse_train_type(train_id),
        from_station=from_station,
        to_station=to_station,
        from_city=from_city,
        to_city=to_city,
        depart_time=depart_time,
        arrive_time=arrive_time,
        duration_minutes=parse_duration(duration_str),
        distance_km=None,
        seats=seats,
    )


async def fetch_trains(
    from_city: str,
    to_city: str,
    travel_date: str,
) -> List[ScrapedTrain]:
    """Fetch train tickets from 12306 JSON API.

    Uses httpx to call 12306's queryZ endpoint directly — no browser needed.
    Raises on failure (no mock fallback).
    """
    # Ensure station codes are loaded
    await ensure_stations_loaded()

    from_code = get_station_code(from_city)
    to_code = get_station_code(to_city)

    if not from_code:
        raise ValueError(f"无法识别出发城市/车站: {from_city}")
    if not to_code:
        raise ValueError(f"无法识别到达城市/车站: {to_city}")

    date_obj = datetime.strptime(travel_date, "%Y-%m-%d")
    formatted_date = date_obj.strftime("%Y-%m-%d")

    params = {
        "leftTicketDTO.train_date": formatted_date,
        "leftTicketDTO.from_station": from_code,
        "leftTicketDTO.to_station": to_code,
        "purpose_codes": "ADULT",
    }

    logger.info(
        "Querying 12306 API: %s(%s) -> %s(%s) on %s",
        from_city, from_code, to_city, to_code, formatted_date,
    )

    async with httpx.AsyncClient(
        headers=BROWSER_HEADERS,
        follow_redirects=True,
        timeout=30.0,
    ) as client:
        # Visit init page first to obtain session cookies
        await client.get(INIT_URL)

        # Query the API with session cookies
        resp = await client.get(QUERY_URL, params=params, headers=API_HEADERS)

        if resp.status_code != 200:
            raise RuntimeError(
                f"12306 API 返回状态码 {resp.status_code}: {resp.text[:200]}"
            )

        try:
            data = resp.json()
        except Exception:
            raise RuntimeError(
                f"12306 返回非 JSON 响应 (可能被反爬拦截): {resp.text[:200]}"
            )

    # Check for API-level errors
    if not data.get("status"):
        messages = data.get("messages", [])
        msg = "; ".join(messages) if messages else "未知错误"
        raise RuntimeError(f"12306 API 查询失败: {msg}")

    result_data = data.get("data", {})
    result_list = result_data.get("result", [])
    station_map = result_data.get("map", {})

    if not result_list:
        logger.info("12306 returned 0 trains for %s -> %s on %s", from_city, to_city, formatted_date)
        return []

    trains: List[ScrapedTrain] = []
    for raw in result_list:
        train = _parse_train_result(raw, station_map, from_city, to_city)
        if train:
            trains.append(train)

    logger.info("Fetched %d trains from 12306 API", len(trains))
    return trains
