"""Flight ticket fetcher using Ctrip H5 API."""
from __future__ import annotations

import asyncio
import logging
from typing import Dict, List, Optional, Set, Tuple

import httpx

from ..utils.geo_lookup import get_airport_code
from .types import ScrapedFlight, FlightCabinPrice

logger = logging.getLogger(__name__)

# Ctrip H5 (mobile) flight search API
CTRIP_H5_API = "https://m.ctrip.com/restapi/soa2/14022/flightListSearch"

CTRIP_H5_HEADERS = {
    "User-Agent": (
        "Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) "
        "AppleWebKit/605.1.15 (KHTML, like Gecko) "
        "Version/16.0 Mobile/15E148 Safari/604.1"
    ),
    "Content-Type": "application/json",
    "Accept": "application/json",
    "Origin": "https://m.ctrip.com",
    "Referer": "https://m.ctrip.com/html5/flight/swift/domestic/list",
}

# City name -> Ctrip airport city code mapping for common cities
# Ctrip uses 3-letter codes: BJS=北京, SHA=上海, CAN=广州, SZX=深圳, etc.
_CITY_CODES = {
    "北京": "BJS", "上海": "SHA", "广州": "CAN", "深圳": "SZX",
    "成都": "CTU", "杭州": "HGH", "重庆": "CKG", "武汉": "WUH",
    "西安": "SIA", "南京": "NKG", "长沙": "CSX", "昆明": "KMG",
    "厦门": "XMN", "天津": "TSN", "郑州": "CGO", "青岛": "TAO",
    "大连": "DLC", "哈尔滨": "HRB", "沈阳": "SHE", "三亚": "SYX",
    "海口": "HAK", "福州": "FOC", "贵阳": "KWE", "南宁": "NNG",
    "兰州": "LHW", "太原": "TYN", "合肥": "HFE", "长春": "CGQ",
    "济南": "TNA", "南昌": "KHN", "乌鲁木齐": "URC", "呼和浩特": "HET",
    "石家庄": "SJW", "银川": "INC", "拉萨": "LXA", "西宁": "XNN",
    "珠海": "ZUH", "温州": "WNZ", "宁波": "NGB", "无锡": "WUX",
    "烟台": "YNT", "桂林": "KWL", "丽江": "LJG", "遵义": "ZYI",
}


def _resolve_ctrip_city_code(city: str) -> str:
    """Resolve city name to Ctrip city code.

    Falls back to IATA airport code from the DB if city is not in the mapping.
    """
    code = _CITY_CODES.get(city)
    if code:
        return code
    # Try the airport code from DB
    airport = get_airport_code(city)
    if airport and airport != "UNK":
        return airport
    raise ValueError(f"无法识别城市: {city}")


def _parse_ctrip_flight(item: dict, from_city: str, to_city: str) -> Optional[ScrapedFlight]:
    """Parse a single flight item from the Ctrip H5 API response."""
    try:
        # Flight segments
        mutli_flts = item.get("mutilstn", [])
        if not mutli_flts:
            return None

        seg = mutli_flts[0]
        flight_id = seg.get("fltno", "")
        airline = seg.get("aln", "")
        from_airport = seg.get("dpbn", "") or seg.get("dport", "")
        to_airport = seg.get("apbn", "") or seg.get("aport", "")
        depart_time = seg.get("dtm", "")[:5] if seg.get("dtm") else ""
        arrive_time = seg.get("atm", "")[:5] if seg.get("atm") else ""
        aircraft_type = seg.get("craft", None)

        # Duration
        duration_minutes = seg.get("duration", 0)

        # Prices
        prices: List[FlightCabinPrice] = []
        cabin_info = item.get("pricelist", [])
        if not cabin_info:
            # Try alternate price fields
            economy_price = item.get("prc", 0) or item.get("eco", {}).get("p", 0)
            if economy_price:
                prices.append(FlightCabinPrice(
                    cabin_class="经济舱",
                    price=float(economy_price),
                    discount=item.get("rat", None),
                    available_seats=None,
                ))
        else:
            cabin_map = {"Y": "经济舱", "C": "商务舱", "F": "头等舱", "S": "超级经济舱"}
            for p_item in cabin_info:
                cabin_code = p_item.get("cabin", "Y")
                cabin_name = cabin_map.get(cabin_code, cabin_code)
                price_val = p_item.get("price", 0) or p_item.get("prc", 0)
                if not price_val:
                    continue
                prices.append(FlightCabinPrice(
                    cabin_class=cabin_name,
                    price=float(price_val),
                    discount=p_item.get("rat", None),
                    available_seats=p_item.get("seatcnt", None),
                ))

        if not flight_id:
            return None

        return ScrapedFlight(
            flight_id=flight_id,
            airline=airline,
            from_airport=from_airport,
            to_airport=to_airport,
            from_city=from_city,
            to_city=to_city,
            depart_time=depart_time,
            arrive_time=arrive_time,
            duration_minutes=duration_minutes,
            aircraft_type=aircraft_type,
            source="ctrip",
            prices=prices,
        )
    except Exception as e:
        logger.warning("Failed to parse Ctrip flight item: %s", e)
        return None


async def fetch_flights_ctrip(
    from_city: str,
    to_city: str,
    travel_date: str,
) -> List[ScrapedFlight]:
    """Fetch flights from Ctrip H5 API.

    Uses Ctrip's mobile SOA API which returns structured JSON.
    Polls up to 3 times since the API uses an async search pattern.
    """
    from_code = _resolve_ctrip_city_code(from_city)
    to_code = _resolve_ctrip_city_code(to_city)

    logger.info(
        "Querying Ctrip H5 API: %s(%s) -> %s(%s) on %s",
        from_city, from_code, to_city, to_code, travel_date,
    )

    payload = {
        "contentType": "json",
        "flag": 8,
        "flightWay": "S",
        "hasChild": False,
        "hasBaby": False,
        "searchIndex": 1,
        "airportParams": [{
            "dcity": from_code,
            "acity": to_code,
            "date": travel_date,
        }],
    }

    all_flights: List[ScrapedFlight] = []

    async with httpx.AsyncClient(
        headers=CTRIP_H5_HEADERS,
        follow_redirects=True,
        timeout=30.0,
    ) as client:
        token = ""
        for attempt in range(1, 4):
            if token:
                payload["token"] = token
            payload["searchIndex"] = attempt

            resp = await client.post(CTRIP_H5_API, json=payload)
            if resp.status_code != 200:
                logger.warning("Ctrip API returned %d on attempt %d", resp.status_code, attempt)
                break

            try:
                data = resp.json()
            except Exception:
                logger.warning("Ctrip returned non-JSON on attempt %d", attempt)
                break

            fltitem = data.get("fltitem", [])
            token = data.get("token", "") or token
            is_complete = data.get("iscomplete", False)

            for item in fltitem:
                flight = _parse_ctrip_flight(item, from_city, to_city)
                if flight:
                    all_flights.append(flight)

            logger.info(
                "Ctrip poll %d: %d flights, complete=%s",
                attempt, len(fltitem), is_complete,
            )

            if is_complete or fltitem:
                break

            await asyncio.sleep(1.0)

    logger.info("Ctrip returned %d flights total", len(all_flights))
    return all_flights


# ============================================================
# Multi-source aggregation
# ============================================================

def _merge_flights(all_results: List[Tuple[str, List[ScrapedFlight]]]) -> List[ScrapedFlight]:
    """Merge flights from multiple sources, deduplicating by flight_id."""
    flight_map: Dict[str, ScrapedFlight] = {}
    flight_sources: Dict[str, Set[str]] = {}
    cabin_prices: Dict[str, Dict[str, FlightCabinPrice]] = {}

    for source_name, flights in all_results:
        for f in flights:
            fid = f.flight_id

            if fid not in flight_map:
                flight_map[fid] = f
                flight_sources[fid] = {source_name}
                cabin_prices[fid] = {}
            else:
                flight_sources[fid].add(source_name)

            for p in f.prices:
                existing = cabin_prices[fid].get(p.cabin_class)
                if existing is None or p.price < existing.price:
                    cabin_prices[fid][p.cabin_class] = p

    merged: List[ScrapedFlight] = []
    for fid, flight in flight_map.items():
        sources = sorted(flight_sources[fid])
        best_prices = list(cabin_prices[fid].values())
        best_prices.sort(key=lambda p: p.price)
        merged.append(ScrapedFlight(
            flight_id=flight.flight_id,
            airline=flight.airline,
            from_airport=flight.from_airport,
            to_airport=flight.to_airport,
            from_city=flight.from_city,
            to_city=flight.to_city,
            depart_time=flight.depart_time,
            arrive_time=flight.arrive_time,
            duration_minutes=flight.duration_minutes,
            aircraft_type=flight.aircraft_type,
            source=",".join(sources),
            prices=best_prices,
        ))

    merged.sort(key=lambda f: min((p.price for p in f.prices), default=float("inf")))
    return merged


# ============================================================
# Main entry point
# ============================================================

async def fetch_flights(
    from_city: str,
    to_city: str,
    travel_date: str,
) -> List[ScrapedFlight]:
    """Fetch flights from Ctrip. No mock fallback — raises on total failure."""
    # Currently only Ctrip source is implemented.
    # When more real sources are added (Qunar, Fliggy), they can be
    # wired into multi-source aggregation here.
    flights = await fetch_flights_ctrip(from_city, to_city, travel_date)
    if flights:
        return flights

    logger.warning(
        "未获取到航班数据: %s -> %s on %s",
        from_city, to_city, travel_date,
    )
    return []
