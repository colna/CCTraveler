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

from ..anti_detect.fingerprint import pick_user_agent
from ..anti_detect.proxy import pick_proxy
from ..utils.geo_lookup import get_airport_code
from .types import ScrapedFlight, FlightCabinPrice

logger = logging.getLogger(__name__)

_BACKOFF_BASE = 2.0
_BACKOFF_CAP = 30.0

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

# Regex patterns for parsing price / discount from innerText
_RE_PRICE = re.compile(r"[¥￥](\d+)")
_RE_AIRCRAFT = re.compile(
    r"((?:空客|波音|商飞|ARJ|CRJ|ERJ|A\d{3}|B\d{3}|737|320|321|330|350|380|787|777|747)\S*)"
)

# JS script to extract structured flight data from the Ctrip page DOM.
# Uses element IDs to get flight numbers (many items don't show them in text),
# and specific CSS selectors for airline, times, airports.
_EXTRACT_JS = """() => {
    const items = document.querySelectorAll('.flight-item');
    return Array.from(items).map(item => {
        // Flight number: hidden in child element IDs like "airlineNameKN5955_..."
        let flightNo = '';
        const idEls = item.querySelectorAll('[id]');
        for (const el of idEls) {
            const m = el.id.match(/airlineName([A-Z0-9]{2,3}\\d{3,4})_/);
            if (m) { flightNo = m[1]; break; }
        }
        if (!flightNo) return null;

        // Transfer: check transfer-text element
        for (const el of idEls) {
            if (el.id.startsWith('transfer-text-')) {
                const t = el.textContent.trim();
                if (t && t.length > 0) return null;  // skip transfer flights
                break;
            }
        }

        // Airline name
        const airlineEl = item.querySelector('.airline-name');
        const airline = airlineEl ? airlineEl.textContent.trim() : '';

        // Departure
        const dTimeEl = item.querySelector('.depart-box .time');
        const departTime = dTimeEl ? dTimeEl.childNodes[0].textContent.trim() : '';
        const dAirportEl = item.querySelector('.depart-box .airport .name');
        const dTerminalEl = item.querySelector('.depart-box .terminal');
        const departAirport = (dAirportEl ? dAirportEl.textContent.trim() : '')
            + (dTerminalEl ? dTerminalEl.textContent.trim() : '');

        // Arrival
        const aTimeEl = item.querySelector('.arrive-box .time');
        const arriveTime = aTimeEl ? aTimeEl.childNodes[0].textContent.trim() : '';
        const aAirportEl = item.querySelector('.arrive-box .airport .name');
        const aTerminalEl = item.querySelector('.arrive-box .terminal');
        const arriveAirport = (aAirportEl ? aAirportEl.textContent.trim() : '')
            + (aTerminalEl ? aTerminalEl.textContent.trim() : '');

        // Full text for price/discount/aircraft extraction
        const fullText = item.innerText;

        return {
            flightNo, airline,
            departTime, departAirport,
            arriveTime, arriveAirport,
            fullText
        };
    }).filter(x => x !== null);
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


def _build_flight(
    item: dict,
    from_city: str,
    to_city: str,
) -> Optional[ScrapedFlight]:
    """Build a ScrapedFlight from a DOM-extracted dict."""
    flight_id = item.get("flightNo", "")
    if not flight_id:
        return None

    text = item.get("fullText", "")

    # Price — match "¥XXX起" first, then plain "¥XXX"
    price_match = re.search(r"[¥￥](\d+)起", text)
    if not price_match:
        price_match = _RE_PRICE.search(text)
    price = float(price_match.group(1)) if price_match else 0.0

    # Discount — match "经济舱X.X折" to avoid promo text
    discount_val: Optional[float] = None
    cabin_discount = re.search(
        r"(?:经济舱|商务舱|头等舱|超级经济舱)(\d+\.?\d*)折", text,
    )
    if cabin_discount:
        discount_val = float(cabin_discount.group(1)) / 10

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

    depart_time = item.get("departTime", "")
    arrive_time = item.get("arriveTime", "")

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
        airline=item.get("airline", ""),
        from_airport=item.get("departAirport", ""),
        to_airport=item.get("arriveAirport", ""),
        from_city=from_city,
        to_city=to_city,
        depart_time=depart_time,
        arrive_time=arrive_time,
        duration_minutes=_calc_duration(depart_time, arrive_time),
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

    Launches headless Chromium, loads the flight list page, scrolls to
    trigger lazy loading, and extracts data from DOM structure.
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

    attempt = 0
    while True:
        attempt += 1
        proxy = pick_proxy()
        launch_kwargs = {"headless": True}
        if proxy:
            launch_kwargs["proxy"] = {"server": proxy}

        try:
            async with async_playwright() as pw:
                browser = await pw.chromium.launch(**launch_kwargs)
                try:
                    context = await browser.new_context(
                        user_agent=pick_user_agent(),
                        viewport={"width": 1920, "height": 1080},
                        locale="zh-CN",
                    )
                    page = await context.new_page()

                    await page.goto(url, wait_until="domcontentloaded", timeout=30000)

                    # Wait for flight items to render
                    try:
                        await page.wait_for_selector(".flight-item", timeout=20000)
                    except Exception:
                        title = await page.title()
                        logger.warning(
                            "No .flight-item elements found (page title: %s) "
                            "— attempt %d", title, attempt,
                        )
                        # Treat as a soft block: retry with new UA/proxy.
                        raise RuntimeError(f"no flight-item rendered (title={title!r})")

                    # Give the page a moment to finish rendering initial items
                    await asyncio.sleep(3)

                    # Scroll repeatedly to trigger lazy loading until all flights appear
                    prev_count = 0
                    for _ in range(15):
                        await page.evaluate("window.scrollTo(0, document.body.scrollHeight)")
                        await asyncio.sleep(1.5)
                        cur_count = await page.evaluate(
                            "document.querySelectorAll('.flight-item').length",
                        )
                        if cur_count == prev_count:
                            break
                        prev_count = cur_count

                    # Extract structured data from DOM
                    raw_items: List[dict] = await page.evaluate(_EXTRACT_JS)
                    logger.info(
                        "Found %d valid .flight-item elements in DOM", len(raw_items),
                    )

                    seen_ids: Set[str] = set()
                    for item in raw_items:
                        flight = _build_flight(item, from_city, to_city)
                        if flight and flight.flight_id not in seen_ids:
                            seen_ids.add(flight.flight_id)
                            flights.append(flight)
                finally:
                    await browser.close()
            break  # success — exit retry loop
        except Exception as e:
            sleep_for = min(_BACKOFF_BASE ** attempt, _BACKOFF_CAP)
            logger.warning(
                "Playwright flight fetch attempt %d failed: %s — retrying in %.1fs",
                attempt, e, sleep_for,
            )
            await asyncio.sleep(sleep_for)

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
