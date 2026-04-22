"""Parse hotel list pages from Ctrip / Trip.com into structured data.

Both sites use Next.js RSC. Hotel data is embedded as escaped JSON.

Ctrip:    initListData.hotelList[] — hotelInfo structure, NO prices in SSR.
Trip.com: standalone hotelList[]   — may use hotelBasicInfo (flat, with price)
          or hotelInfo (nested, with roomInfo[].priceInfo).
"""
from __future__ import annotations

import json
import logging
import re
from typing import Optional, List, Dict, Set

from .types import ScrapedHotel, ScrapedRoom

logger = logging.getLogger(__name__)


def parse_hotel_list(html: str, city: str) -> List[ScrapedHotel]:
    """Parse hotel list page HTML and extract hotel data."""
    unescaped = html.replace('\\"', '"').replace('\\\\', '\\')

    hotels_data = _extract_hotel_list(unescaped)
    if not hotels_data:
        logger.warning("Could not extract hotelList from page")
        return []

    hotels: List[ScrapedHotel] = []
    seen: Set[str] = set()

    for item in hotels_data:
        try:
            hotel = _convert_hotel(item, city)
            if hotel and hotel.id not in seen:
                seen.add(hotel.id)
                hotels.append(hotel)
        except Exception:
            logger.debug("Failed to convert hotel item", exc_info=True)

    logger.info("Parsed %d hotels from page", len(hotels))
    return hotels


def _extract_hotel_list(text: str) -> List[Dict]:
    """Extract hotelList array from the page.

    Tries multiple strategies:
    1. initListData.hotelList (Ctrip)
    2. Standalone "hotelList":[ (Trip.com)
    """
    # Strategy 1: Ctrip — initListData wrapper
    m = re.search(r'"initListData"\s*:\s*\{', text)
    if m:
        pos = m.start() + len('"initListData":')
        blob = _extract_json_object(text, pos)
        if blob:
            try:
                data = json.loads(blob)
                hotel_list = data.get("hotelList", [])
                if hotel_list:
                    return hotel_list
            except json.JSONDecodeError:
                pass

    # Strategy 2: Trip.com — standalone hotelList array
    for m in re.finditer(r'"hotelList"\s*:\s*\[', text):
        arr_start = text.index('[', m.start())
        arr_json = _extract_json_array(text, arr_start)
        if arr_json:
            try:
                result = json.loads(arr_json)
                if result and isinstance(result, list) and len(result) > 0:
                    # Verify it looks like hotel data (not some other list)
                    first = result[0]
                    if isinstance(first, dict) and (
                        "hotelBasicInfo" in first or "hotelInfo" in first
                    ):
                        return result
            except json.JSONDecodeError:
                continue

    return []


def _extract_json_object(text: str, pos: int) -> Optional[str]:
    """Extract a JSON object starting at pos (finds first { and matches braces)."""
    start = text.find('{', pos)
    if start < 0 or start > pos + 10:
        return None
    depth = 0
    for i in range(start, min(start + 2_000_000, len(text))):
        if text[i] == '{':
            depth += 1
        elif text[i] == '}':
            depth -= 1
            if depth == 0:
                return text[start:i + 1]
    return None


def _extract_json_array(text: str, pos: int) -> Optional[str]:
    """Extract a JSON array starting at pos."""
    depth = 0
    for i in range(pos, min(pos + 2_000_000, len(text))):
        if text[i] == '[':
            depth += 1
        elif text[i] == ']':
            depth -= 1
            if depth == 0:
                return text[pos:i + 1]
    return None


def _convert_hotel(item: dict, city: str) -> Optional[ScrapedHotel]:
    """Convert a hotel item — auto-detects structure variant."""
    if "hotelBasicInfo" in item:
        return _convert_hotel_basic(item, city)
    if "hotelInfo" in item:
        return _convert_hotel_nested(item, city)
    return None


def _convert_hotel_basic(item: dict, city: str) -> Optional[ScrapedHotel]:
    """Convert Trip.com flat structure (hotelBasicInfo with embedded price)."""
    bi = item.get("hotelBasicInfo", {})

    hotel_id = str(bi.get("hotelId", ""))
    if not hotel_id:
        return None

    name = bi.get("hotelName", "")
    if not name:
        return None

    # Star
    star = _to_int(bi.get("superStar")) or _to_int(
        item.get("hotelStarInfo", {}).get("star")
    )

    # Comment / rating
    ci = item.get("commentInfo", {})
    rating = _to_float(ci.get("commentScore"))
    rating_count = _parse_comment_count(ci.get("commentCount", "") or ci.get("commentDescription", ""))

    # Position
    pi = item.get("positionInfo", {})
    address = bi.get("hotelAddress") or pi.get("positionDesc")
    city_name = pi.get("cityName", city)
    zone_names = pi.get("zoneNames", [])
    district = zone_names[0] if zone_names else None

    lat, lng = None, None
    coords = pi.get("coordinate", {})
    if coords:
        lat = _to_float(coords.get("lat"))
        lng = _to_float(coords.get("lng"))

    # Image
    image_url = bi.get("hotelImg")

    # Price from hotelBasicInfo (flat structure)
    price = _to_float(bi.get("price"))
    original_price = _to_float(bi.get("originPrice"))

    # Room info (may have detailed room data)
    rooms: List[ScrapedRoom] = []
    room_info = item.get("roomInfo", item.get("minRoomInfo", {}))
    if isinstance(room_info, dict):
        room_name = room_info.get("roomName") or room_info.get("physicsName") or "Room"
        bed_list = room_info.get("bedInfo", {}).get("contentList", [])
        bed_type = bed_list[0] if bed_list else None
        rooms.append(ScrapedRoom(
            name=room_name,
            price=price,
            original_price=original_price,
            currency="CNY",
            bed_type=bed_type,
            has_free_cancel=bi.get("isFreeCancelOfMinRoom", False),
        ))
    elif isinstance(room_info, list):
        for ri in room_info:
            _add_room(ri, rooms, fallback_price=price, fallback_orig=original_price)
    else:
        # No room details — create a placeholder with hotel-level price
        rooms.append(ScrapedRoom(
            name="Standard Room",
            price=price,
            original_price=original_price,
            currency="CNY",
        ))

    return ScrapedHotel(
        id=hotel_id,
        name=name,
        name_en=bi.get("hotelEnName"),
        star=star,
        rating=rating,
        rating_count=rating_count,
        address=address,
        latitude=lat,
        longitude=lng,
        image_url=image_url,
        city=city_name,
        district=district,
        rooms=rooms,
    )


def _convert_hotel_nested(item: dict, city: str) -> Optional[ScrapedHotel]:
    """Convert Ctrip/Trip.com nested structure (hotelInfo + roomInfo[])."""
    info = item.get("hotelInfo", {})
    summary = info.get("summary", {})

    hotel_id = str(summary.get("hotelId", ""))
    if not hotel_id:
        return None

    name_info = info.get("nameInfo", {})
    name = name_info.get("name", "")
    if not name:
        return None

    star_info = info.get("hotelStar", {})
    star = _to_int(star_info.get("star"))

    comment_info = info.get("commentInfo", {})
    rating = _to_float(comment_info.get("commentScore"))
    rating_count_str = comment_info.get("commenterNumber", "")
    rating_count = _parse_comment_count(rating_count_str)

    pos_info = info.get("positionInfo", {})
    address = pos_info.get("address")
    city_name = pos_info.get("cityName", city)
    district_names = pos_info.get("zoneNames", [])
    district = district_names[0] if district_names else None

    lat, lng = None, None
    coords = pos_info.get("mapCoordinate", [])
    if coords:
        lat = _to_float(coords[0].get("latitude"))
        lng = _to_float(coords[0].get("longitude"))

    images = info.get("hotelImages", {}).get("multiImgs", [])
    image_url = images[0].get("url") if images else None

    rooms: list[ScrapedRoom] = []
    for room_item in item.get("roomInfo", []):
        _add_room(room_item, rooms)

    return ScrapedHotel(
        id=hotel_id,
        name=name,
        name_en=name_info.get("enName"),
        star=star,
        rating=rating,
        rating_count=rating_count,
        address=address,
        latitude=lat,
        longitude=lng,
        image_url=image_url,
        city=city_name,
        district=district,
        rooms=rooms,
    )


def _add_room(
    room_item: dict,
    rooms: List[ScrapedRoom],
    fallback_price: Optional[float] = None,
    fallback_orig: Optional[float] = None,
) -> None:
    """Extract a room from roomInfo item and append to rooms list."""
    room_summary = room_item.get("summary", {})
    room_name = (
        room_summary.get("saleRoomName")
        or room_summary.get("physicsName")
        or room_item.get("roomName")
        or "Room"
    )

    bed_info = room_item.get("bedInfo", {})
    bed_list = bed_info.get("contentList", [])
    bed_type = bed_list[0] if bed_list else None

    # Extract price from priceInfo (Trip.com provides this)
    price_info = room_item.get("priceInfo", {})
    price = _to_float(price_info.get("price")) or fallback_price
    original_price = _to_float(price_info.get("deletePrice")) or fallback_orig
    currency = price_info.get("currency", "CNY")

    # Free cancellation — check both Chinese and English tags
    tags = room_item.get("roomTags", {}).get("advantageTags", [])
    has_free_cancel = any(
        "免费取消" in t.get("tagTitle", "") or "Free Cancel" in t.get("tagTitle", "")
        for t in tags
    )

    has_breakfast = any(
        "早餐" in t.get("tagTitle", "") or "Breakfast" in t.get("tagTitle", "")
        for t in tags
    )

    rooms.append(ScrapedRoom(
        name=room_name,
        price=price,
        original_price=original_price,
        currency=currency,
        bed_type=bed_type,
        has_breakfast=has_breakfast if has_breakfast else None,
        has_free_cancel=has_free_cancel,
    ))


def _parse_comment_count(s: str) -> Optional[int]:
    """Parse '1,417条点评' or '9,909 reviews' → integer."""
    if not s:
        return None
    nums = re.findall(r"[\d,]+", s)
    if nums:
        try:
            return int(nums[0].replace(",", ""))
        except ValueError:
            pass
    return None


def _to_float(val) -> Optional[float]:
    if val is None or val == "":
        return None
    try:
        return float(val)
    except (ValueError, TypeError):
        return None


def _to_int(val) -> Optional[int]:
    if val is None:
        return None
    try:
        return int(val)
    except (ValueError, TypeError):
        return None
