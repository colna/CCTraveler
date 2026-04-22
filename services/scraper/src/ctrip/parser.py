"""Parse Ctrip hotel list page into structured data.

Ctrip uses Next.js RSC (React Server Components). Hotel data is embedded
in the page as escaped JSON inside self.__next_f.push() calls.
The data lives under `initListData.hotelList[]`.

Price is NOT included in SSR — it loads via client-side JS.
We extract all available info (name, star, rating, location, rooms)
and mark price as None for now.
"""
from __future__ import annotations

import json
import logging
import re

from .types import ScrapedHotel, ScrapedRoom

logger = logging.getLogger(__name__)


def parse_hotel_list(html: str, city: str) -> list[ScrapedHotel]:
    """Parse hotel list page HTML and extract hotel data."""
    # Ctrip RSC payload has escaped JSON — unescape it
    unescaped = html.replace('\\"', '"').replace('\\\\', '\\')

    # Extract initListData JSON blob
    hotels_data = _extract_init_list_data(unescaped)
    if not hotels_data:
        logger.warning("Could not extract initListData from page")
        return []

    hotels: list[ScrapedHotel] = []
    seen: set[str] = set()

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


def _extract_init_list_data(text: str) -> list[dict]:
    """Extract hotelList array from initListData in the RSC payload."""
    m = re.search(r'"initListData"\s*:\s*\{', text)
    if not m:
        return []

    pos = m.start() + len('"initListData":')
    depth = 0
    end = pos

    for i in range(pos, min(pos + 1_000_000, len(text))):
        if text[i] == '{':
            depth += 1
        elif text[i] == '}':
            depth -= 1
            if depth == 0:
                end = i + 1
                break

    try:
        data = json.loads(text[pos:end])
        return data.get("hotelList", [])
    except (json.JSONDecodeError, KeyError):
        logger.debug("Failed to parse initListData JSON")
        return []


def _convert_hotel(item: dict, city: str) -> ScrapedHotel | None:
    """Convert a single hotel entry from Ctrip's data structure.

    Structure:
    {
      "hotelInfo": {
        "summary": {"hotelId": "...", ...},
        "nameInfo": {"name": "...", "enName": "..."},
        "hotelStar": {"star": 4},
        "commentInfo": {"commentScore": "4.8", "commenterNumber": "1,417条点评"},
        "positionInfo": {"address": "...", "cityName": "...", "mapCoordinate": [...]},
        "hotelImages": {"multiImgs": [{"url": "..."}]},
      },
      "roomInfo": [
        {
          "summary": {"saleRoomName": "..."},
          "bedInfo": {"contentList": ["2张1.2米单人床"]},
          "roomTags": {"advantageTags": [{"tagTitle": "免费取消"}]},
        }
      ]
    }
    """
    info = item.get("hotelInfo", {})
    summary = info.get("summary", {})

    hotel_id = str(summary.get("hotelId", ""))
    if not hotel_id:
        return None

    name_info = info.get("nameInfo", {})
    name = name_info.get("name", "")
    if not name:
        return None

    # Star rating
    star_info = info.get("hotelStar", {})
    star = _to_int(star_info.get("star"))

    # Comment / rating
    comment_info = info.get("commentInfo", {})
    rating = _to_float(comment_info.get("commentScore"))
    rating_count_str = comment_info.get("commenterNumber", "")
    rating_count = _parse_comment_count(rating_count_str)

    # Position
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

    # Image
    images = info.get("hotelImages", {}).get("multiImgs", [])
    image_url = images[0].get("url") if images else None

    # Rooms
    rooms: list[ScrapedRoom] = []
    for room_item in item.get("roomInfo", []):
        room_summary = room_item.get("summary", {})
        room_name = room_summary.get("saleRoomName") or room_summary.get("physicsName") or "Room"

        bed_info = room_item.get("bedInfo", {})
        bed_list = bed_info.get("contentList", [])
        bed_type = bed_list[0] if bed_list else None

        # Check for free cancellation
        tags = room_item.get("roomTags", {}).get("advantageTags", [])
        has_free_cancel = any("免费取消" in t.get("tagTitle", "") for t in tags)

        rooms.append(ScrapedRoom(
            name=room_name,
            price=None,  # Price not available in SSR
            original_price=None,
            bed_type=bed_type,
            has_breakfast=None,
            has_free_cancel=has_free_cancel,
        ))

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


def _parse_comment_count(s: str) -> int | None:
    """Parse '1,417条点评' → 1417."""
    if not s:
        return None
    nums = re.findall(r"[\d,]+", s)
    if nums:
        try:
            return int(nums[0].replace(",", ""))
        except ValueError:
            pass
    return None


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
