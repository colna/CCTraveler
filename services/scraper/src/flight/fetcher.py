"""Flight ticket fetcher using Playwright browser automation.

Loads Ctrip's desktop flight search page in headless Chromium,
waits for .flight-item elements to render, then extracts structured
data from the DOM.
"""
from __future__ import annotations

import asyncio
import logging
import re
from typing import Dict, List, Optional, Set, Tuple

from ..utils.geo_lookup import get_airport_code
from .types import ScrapedFlight, FlightCabinPrice

logger = logging.getLogger(__name__)

# City name -> Ctrip URL city code
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

CTRIP_FLIGHT_URL = (
    "https://flights.ctrip.com/online/list/oneway-{from_code}-{to_code}"
    "?depdate={date}"
)

# ---- Regex patterns for parsing DOM text ----
_RE_FLIGHT_NO = re.compile(r"\b([A-Z\d]{2}\d{3,4})\b")
_RE_TIME = re.compile(r"(\d{2}:\d{2})")
_RE_PRICE = re.compile(r"[¥￥](\d+)")
_RE_DISCOUNT = re.compile(r"(\d+\.?\d*)折")
_RE_AIRPORT = re.compile(r"([\u4e00-\u9fa5]+机场[T\d]*)")
_RE_AIRCRAFT = re.compile(
    r"((?:空客|波音|商飞|ARJ|CRJ|ERJ|A\d{3}|B\d{3}|737|320|321|330|350|380|787|777|747)\S*)"
)

# JS script to extract structured flight data from the Ctrip page DOM.
# Each .flight-item is walked for text nodes, yielding a pipe-separated string
# that is easier to parse than raw innerText.
_EXTRACT_JS = """() => {
    const items = document.querySelectorAll('.flight-item');
    return Array.from(items).map(item => {
        return item.innerText;
    });
}"""


def _resolve_ctrip_city_code(city: str) -> str:
    """Resolve city name to Ctrip city code."""
    code = _CITY_CODES.get(city)
    if code:
        return code
    airport = get_airport_code(city)
    if airport and airport != "UNK":
        return airport
    raise ValueError(f"无法识别城市: {city}")


def _calc_duration(depart: str, arrive: str) -> int:
    """Calculate duration in minutes from HH:MM strings."""
    try:
        dh, dm = map(int, depart.split(":"))
        ah, am = map(int, arrive.split(":"))
        diff = (ah * 60 + am) - (dh * 60 + dm)
        if diff < 0:
            diff += 24 * 60  # next-day arrival
        return diff
    except (ValueError, IndexError):
        return 0


def _parse_flight_text(
    text: str,
    from_city: str,
    to_city: str,
) -> Optional[ScrapedFlight]:
    """Parse a single .flight-item's innerText into a ScrapedFlight."""
    # Skip transfer/stopover flights
    if "中转" in text or "转机" in text:
        return None

    # Flight number (required)
    flight_match = _RE_FLIGHT_NO.search(text)
    if not flight_match:
        return None
    flight_id = flight_match.group(1)

    # Departure / arrival times
    times = _RE_TIME.findall(text)
    depart_time = times[0] if len(times) >= 1 else ""
    arrive_time = times[1] if len(times) >= 2 else ""

    # Airports
    airports = _RE_AIRPORT.findall(text)
    from_airport = airports[0] if len(airports) >= 1 else ""
    to_airport = airports[1] if len(airports) >= 2 else ""

    # Price — match "¥XXX起" to avoid grabbing promo numbers
    price_match = re.search(r"[¥￥](\d+)起", text)
    if not price_match:
        price_match = _RE_PRICE.search(text)
    price = float(price_match.group(1)) if price_match else 0.0

    # Discount — match "经济舱X.X折" pattern to avoid promo text like "85折优惠券"
    discount_val: Optional[float] = None
    cabin_discount = re.search(
        r"(?:经济舱|商务舱|头等舱|超级经济舱)(\d+\.?\d*)折", text,
    )
    if cabin_discount:
        discount_val = float(cabin_discount.group(1)) / 10

    # Airline — the first non-empty line (usually airline name)
    airline = ""
    for line in text.split("\n"):
        line = line.strip()
        if line and re.search(r"[\u4e00-\u9fa5]", line):
            airline = line
            break

    # Aircraft type
    aircraft_match = _RE_AIRCRAFT.search(text)
    aircraft_type = aircraft_match.group(1).rstrip("共享") if aircraft_match else None

    # Cabin class
    cabin_class = "经济舱"
    if "商务舱" in text:
        cabin_class = "商务舱"
    elif "头等舱" in text:
        cabin_class = "头等舱"
    elif "超级经济舱" in text:
        cabin_class = "超级经济舱"

    # Duration
    duration = _calc_duration(depart_time, arrive_time)

    prices: List[FlightCabinPrice] = []
    if price > 0:
        prices.append(FlightCabinPrice(
            cabin_class=cabin_class,
            price=price,
            discount=discount_val,
            available_seats=None,
        ))

    return ScrapedFlight(
        flight_id=flight_id,
        airline=airline,
        from_airport=from_airport,
        to_airport=to_airport,
        from_city=from_city,
        to_city=to_city,
        depart_time=depart_time,
        arrive_time=arrive_time,
        duration_minutes=duration,
        aircraft_type=aircraft_type,
        source="ctrip",
        prices=prices,
    )


async def fetch_flights_ctrip(
    from_city: str,
    to_city: str,
    travel_date: str,
) -> List[ScrapedFlight]:
    """Fetch flights by rendering Ctrip's flight page with Playwright.

    Launches headless Chromium, loads the flight list page, waits for
    .flight-item DOM elements, and extracts data from their text content.
    """
    from_code = _resolve_ctrip_city_code(from_city)
    to_code = _resolve_ctrip_city_code(to_city)

    url = CTRIP_FLIGHT_URL.format(
        from_code=from_code, to_code=to_code, date=travel_date,
    )
    logger.info(
        "Loading Ctrip flight page: %s(%s) -> %s(%s) on %s",
        from_city, from_code, to_city, to_code, travel_date,
    )

    try:
        from playwright.async_api import async_playwright
    except ImportError:
        logger.error("playwright is not installed — cannot fetch flights")
        return []

    flights: List[ScrapedFlight] = []

    async with async_playwright() as pw:
        browser = await pw.chromium.launch(headless=True)
        try:
            context = await browser.new_context(
                user_agent=(
                    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
                    "AppleWebKit/537.36 (KHTML, like Gecko) "
                    "Chrome/125.0.0.0 Safari/537.36"
                ),
                viewport={"width": 1920, "height": 1080},
                locale="zh-CN",
            )
            page = await context.new_page()

            await page.goto(url, wait_until="domcontentloaded", timeout=30000)

            # Wait for flight items to render
            try:
                await page.wait_for_selector(".flight-item", timeout=20000)
            except Exception:
                # Check for anti-bot / empty page
                title = await page.title()
                logger.warning(
                    "No .flight-item elements found (page title: %s)", title,
                )
                return []

            # Give the page a moment to finish rendering all items
            await asyncio.sleep(3)

            # Scroll down to trigger lazy-loaded items
            await page.evaluate("window.scrollTo(0, document.body.scrollHeight)")
            await asyncio.sleep(2)

            # Extract text from each flight item
            raw_texts: List[str] = await page.evaluate(_EXTRACT_JS)
            logger.info("Found %d .flight-item elements in DOM", len(raw_texts))

            seen_ids: Set[str] = set()
            for text in raw_texts:
                flight = _parse_flight_text(text, from_city, to_city)
                if flight and flight.flight_id not in seen_ids:
                    seen_ids.add(flight.flight_id)
                    flights.append(flight)

        except Exception as e:
            logger.exception("Playwright flight fetch failed: %s", e)
        finally:
            await browser.close()

    logger.info("Extracted %d unique flights from Ctrip", len(flights))
    return flights


# ============================================================
# Main entry point
# ============================================================

async def fetch_flights(
    from_city: str,
    to_city: str,
    travel_date: str,
) -> List[ScrapedFlight]:
    """Fetch flights from Ctrip. No mock fallback — returns [] on failure."""
    flights = await fetch_flights_ctrip(from_city, to_city, travel_date)
    if flights:
        return flights

    logger.warning(
        "未获取到航班数据: %s -> %s on %s",
        from_city, to_city, travel_date,
    )
    return []
