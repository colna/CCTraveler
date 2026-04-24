"""12306 train ticket fetcher using JSON API (no browser needed)."""
from __future__ import annotations

import asyncio
import logging
import re
from datetime import datetime
from typing import Dict, List, Optional, Tuple

import httpx

from ..utils.geo_lookup import get_station_code
from ..utils.station_loader import ensure_stations_loaded
from .types import ScrapedTrain, TrainSeatPrice

logger = logging.getLogger(__name__)

INIT_URL = "https://kyfw.12306.cn/otn/leftTicket/init?linktypeid=dc"
QUERY_URL = "https://kyfw.12306.cn/otn/leftTicket/queryZ"
PRICE_URL = "https://kyfw.12306.cn/otn/leftTicket/queryTicketPrice"

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
_IDX_TRAIN_INTERNAL = 2  # internal train number for price query
_IDX_TRAIN_NO = 3        # display train number e.g. G1234
_IDX_FROM_CODE = 6       # boarding station telegraph code
_IDX_TO_CODE = 7         # alighting station telegraph code
_IDX_DEPART = 8          # departure time
_IDX_ARRIVE = 9          # arrival time
_IDX_DURATION = 10       # duration e.g. "04:30"
_IDX_FROM_SEQ = 16       # boarding station sequence number (for price query)
_IDX_TO_SEQ = 17         # alighting station sequence number (for price query)
_IDX_SEAT_TYPES = 35     # seat type codes (for price query)

# Seat availability field indices in queryZ result
_SEAT_AVAIL_FIELDS = {
    "商务座": 32,
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

# 12306 price API response key -> Chinese seat type name
_PRICE_CODE_MAP = {
    "A9": "商务座",
    "M": "一等座",
    "O": "二等座",
    "A6": "高级软卧",
    "A4": "软卧",
    "F": "动卧",
    "A3": "硬卧",
    "A2": "软座",
    "A1": "硬座",
    "WZ": "无座",
    "MIN": None,  # skip min price field
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


def _parse_price(raw: str) -> Optional[float]:
    """Parse a price string like '¥626.0' to float."""
    raw = raw.strip()
    if raw.startswith("¥"):
        raw = raw[1:]
    try:
        return float(raw)
    except (ValueError, TypeError):
        return None


def _build_seats(
    fields: List[str],
    price_data: Optional[Dict[str, str]],
) -> List[TrainSeatPrice]:
    """Build seat list from availability fields + optional price data."""
    # Gather availability info
    avail_map: Dict[str, Optional[int]] = {}
    for seat_type, idx in _SEAT_AVAIL_FIELDS.items():
        if idx < len(fields):
            avail = _parse_available_seats(fields[idx])
            if avail is not None:
                avail_map[seat_type] = avail

    # Gather price info
    price_map: Dict[str, float] = {}
    if price_data:
        for code, seat_type in _PRICE_CODE_MAP.items():
            if seat_type is None:
                continue
            val = price_data.get(code)
            if val:
                price = _parse_price(val)
                if price and price > 0:
                    price_map[seat_type] = price

    # Merge: use all seat types that have availability OR price
    all_types = set(avail_map.keys()) | set(price_map.keys())

    seats: List[TrainSeatPrice] = []
    for seat_type in all_types:
        seats.append(TrainSeatPrice(
            seat_type=seat_type,
            price=price_map.get(seat_type, 0.0),
            available_seats=avail_map.get(seat_type),
        ))

    return seats


async def _fetch_prices(
    client: httpx.AsyncClient,
    train_rows: List[List[str]],
    travel_date: str,
) -> Dict[str, Dict[str, str]]:
    """Fetch prices for a batch of trains. Returns {train_display_id: price_data}."""
    prices: Dict[str, Dict[str, str]] = {}

    for i, fields in enumerate(train_rows):
        train_id = fields[_IDX_TRAIN_NO]
        train_internal = fields[_IDX_TRAIN_INTERNAL]
        from_seq = fields[_IDX_FROM_SEQ] if len(fields) > _IDX_FROM_SEQ else ""
        to_seq = fields[_IDX_TO_SEQ] if len(fields) > _IDX_TO_SEQ else ""
        seat_types = fields[_IDX_SEAT_TYPES] if len(fields) > _IDX_SEAT_TYPES else ""

        if not (train_internal and from_seq and to_seq and seat_types):
            continue

        try:
            resp = await client.get(PRICE_URL, params={
                "train_no": train_internal,
                "from_station_no": from_seq,
                "to_station_no": to_seq,
                "seat_types": seat_types,
                "train_date": travel_date,
            }, headers=API_HEADERS)

            if resp.status_code != 200:
                logger.debug("Price query returned %d for %s", resp.status_code, train_id)
                continue

            data = resp.json()
            price_data = data.get("data", {})
            # Filter to only price fields (strings starting with ¥ or numeric)
            filtered = {
                k: v for k, v in price_data.items()
                if k not in ("OT", "train_no") and isinstance(v, str) and v
            }
            if filtered:
                prices[train_id] = filtered

        except Exception as e:
            logger.debug("Price query failed for %s: %s", train_id, e)

        # Delay to avoid 12306 rate limiting
        if i + 1 < len(train_rows):
            await asyncio.sleep(0.3)

    logger.info("Fetched prices for %d/%d trains", len(prices), len(train_rows))
    return prices


async def fetch_trains(
    from_city: str,
    to_city: str,
    travel_date: str,
) -> List[ScrapedTrain]:
    """Fetch train tickets from 12306 JSON API with real prices.

    Uses httpx to call 12306's queryZ + queryTicketPrice endpoints.
    Raises on failure (no mock fallback).
    """
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
            logger.info(
                "12306 returned 0 trains for %s -> %s on %s",
                from_city, to_city, formatted_date,
            )
            return []

        # Parse all rows into field lists
        all_fields: List[List[str]] = []
        for raw in result_list:
            fields = raw.split("|")
            if len(fields) >= 34 and fields[_IDX_TRAIN_NO]:
                all_fields.append(fields)

        # Fetch prices for all trains (reuses the same session)
        prices = await _fetch_prices(client, all_fields, formatted_date)

    # Build final train objects
    trains: List[ScrapedTrain] = []
    for fields in all_fields:
        train_id = fields[_IDX_TRAIN_NO]
        from_code_val = fields[_IDX_FROM_CODE]
        to_code_val = fields[_IDX_TO_CODE]

        seats = _build_seats(fields, prices.get(train_id))

        trains.append(ScrapedTrain(
            train_id=train_id,
            train_type=parse_train_type(train_id),
            from_station=station_map.get(from_code_val, from_code_val),
            to_station=station_map.get(to_code_val, to_code_val),
            from_city=from_city,
            to_city=to_city,
            depart_time=fields[_IDX_DEPART],
            arrive_time=fields[_IDX_ARRIVE],
            duration_minutes=parse_duration(fields[_IDX_DURATION]),
            distance_km=None,
            seats=seats,
        ))

    logger.info("Fetched %d trains from 12306 API", len(trains))
    return trains
