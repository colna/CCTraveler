"""Parse Ctrip hotel list HTML into structured data.

NOTE: CSS selectors are speculative and must be validated against real Ctrip HTML.
Ctrip uses React SSR with dynamic class names that may change frequently.
This parser should be treated as a starting point that needs live testing.
"""
from __future__ import annotations

import json
import logging
import re

from .types import ScrapedHotel, ScrapedRoom

logger = logging.getLogger(__name__)


def parse_hotel_list(html: str, city: str) -> list[ScrapedHotel]:
    """Parse hotel list page HTML and extract hotel data.

    Ctrip embeds hotel data as JSON in a <script> tag for SSR hydration.
    We try to extract that JSON first; fall back to HTML parsing if needed.
    """
    # Strategy 1: Extract from embedded JSON (most reliable)
    hotels = _parse_from_json(html, city)
    if hotels:
        return hotels

    # Strategy 2: HTML parsing fallback
    logger.info("JSON extraction failed, falling back to HTML parsing")
    return _parse_from_html(html, city)


def _parse_from_json(html: str, city: str) -> list[ScrapedHotel]:
    """Try to extract hotel data from Ctrip's embedded JSON.

    Ctrip often embeds structured data in script tags like:
    window.__INITIAL_STATE__ = {...}
    """
    patterns = [
        r'window\.__INITIAL_STATE__\s*=\s*(\{.+?\});?\s*</script>',
        r'"hotelList"\s*:\s*(\[.+?\])\s*[,}]',
        r'"hotelPositionJSON"\s*:\s*(\[.+?\])',
    ]

    for pattern in patterns:
        match = re.search(pattern, html, re.DOTALL)
        if not match:
            continue

        try:
            data = json.loads(match.group(1))
            return _normalize_json_hotels(data, city)
        except (json.JSONDecodeError, KeyError, TypeError):
            logger.debug("Pattern '%s' matched but JSON parse failed", pattern[:40])
            continue

    return []


def _normalize_json_hotels(data: dict | list, city: str) -> list[ScrapedHotel]:
    """Normalize extracted JSON into ScrapedHotel objects.

    Handles both the full __INITIAL_STATE__ object and a hotelList array.
    """
    hotel_list: list = []

    if isinstance(data, list):
        hotel_list = data
    elif isinstance(data, dict):
        # Navigate common Ctrip JSON structures
        for path in [
            ["htlsData", "inboundList"],
            ["hotelList"],
            ["result", "hotelList"],
        ]:
            node = data
            for key in path:
                if isinstance(node, dict) and key in node:
                    node = node[key]
                else:
                    node = None
                    break
            if isinstance(node, list) and node:
                hotel_list = node
                break

    hotels: list[ScrapedHotel] = []
    for item in hotel_list:
        try:
            hotel = _convert_json_hotel(item, city)
            if hotel:
                hotels.append(hotel)
        except Exception:
            logger.debug("Failed to convert hotel item", exc_info=True)

    return hotels


def _convert_json_hotel(item: dict, city: str) -> ScrapedHotel | None:
    """Convert a single JSON hotel object to ScrapedHotel."""
    hotel_id = str(item.get("hotelId") or item.get("id") or "")
    name = item.get("hotelName") or item.get("name") or ""

    if not hotel_id or not name:
        return None

    # Extract rooms/prices
    rooms: list[ScrapedRoom] = []
    price_info = item.get("money") or item.get("priceInfo") or item.get("price")
    if isinstance(price_info, (int, float)):
        rooms.append(ScrapedRoom(name="Standard Room", price=float(price_info)))
    elif isinstance(price_info, dict):
        price = price_info.get("price") or price_info.get("amount")
        if price:
            rooms.append(ScrapedRoom(
                name=price_info.get("roomName", "Standard Room"),
                price=float(price),
                original_price=_to_float(price_info.get("originalPrice")),
            ))

    # Also check roomInfo / subRoomList
    for room_item in item.get("roomInfo", []) or item.get("subRoomList", []) or []:
        try:
            room_name = room_item.get("name") or room_item.get("roomName") or "Room"
            room_price = room_item.get("price") or room_item.get("amount")
            if room_price:
                rooms.append(ScrapedRoom(
                    name=room_name,
                    price=float(room_price),
                    original_price=_to_float(room_item.get("originalPrice")),
                    bed_type=room_item.get("bedType"),
                    has_breakfast=room_item.get("hasBreakfast"),
                ))
        except (ValueError, TypeError):
            continue

    return ScrapedHotel(
        id=hotel_id,
        name=name,
        name_en=item.get("hotelNameEn") or item.get("enName"),
        star=_to_int(item.get("star")),
        rating=_to_float(item.get("score") or item.get("rating")),
        rating_count=_to_int(item.get("commentCount") or item.get("reviewCount")),
        address=item.get("address"),
        latitude=_to_float(item.get("lat") or item.get("latitude")),
        longitude=_to_float(item.get("lon") or item.get("longitude")),
        image_url=item.get("pictureUrl") or item.get("imageUrl"),
        city=city,
        district=item.get("zone") or item.get("district"),
        rooms=rooms,
    )


def _parse_from_html(html: str, city: str) -> list[ScrapedHotel]:
    """Fallback: parse hotels from HTML structure.

    This is less reliable as Ctrip uses dynamic class names.
    Kept as a fallback for when JSON extraction fails.
    """
    # Placeholder — requires validation against live Ctrip HTML
    logger.warning("HTML parsing fallback not yet fully implemented")
    return []


def _to_float(val) -> float | None:
    if val is None:
        return None
    try:
        return float(val)
    except (ValueError, TypeError):
        return None


def _to_int(val) -> int | None:
    if val is None:
        return None
    try:
        return int(val)
    except (ValueError, TypeError):
        return None
