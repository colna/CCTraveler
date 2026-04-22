"""Ctrip hotel list fetcher using httpx with browser-like headers."""
from __future__ import annotations

import asyncio
import json
import logging
import random
from pathlib import Path

import httpx

logger = logging.getLogger(__name__)

# Load city lookup from JSON (pinyin + Chinese name -> city ID)
_CITY_LOOKUP: dict[str, int] = {}


def _load_city_lookup() -> dict[str, int]:
    global _CITY_LOOKUP
    if _CITY_LOOKUP:
        return _CITY_LOOKUP
    lookup_path = Path(__file__).resolve().parents[4] / "data" / "city_lookup.json"
    if lookup_path.exists():
        with open(lookup_path, encoding="utf-8") as f:
            _CITY_LOOKUP = json.load(f)
        logger.info("Loaded %d city mappings from %s", len(_CITY_LOOKUP), lookup_path)
    else:
        logger.warning("City lookup file not found: %s", lookup_path)
    return _CITY_LOOKUP


HEADERS = {
    "User-Agent": (
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
        "AppleWebKit/537.36 (KHTML, like Gecko) "
        "Chrome/125.0.0.0 Safari/537.36"
    ),
    "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
    "Accept-Language": "zh-CN,zh;q=0.9,en;q=0.8",
    "Accept-Encoding": "gzip, deflate, br",
    "Cache-Control": "no-cache",
    "Sec-Fetch-Dest": "document",
    "Sec-Fetch-Mode": "navigate",
    "Sec-Fetch-Site": "none",
}


def resolve_city_id(city: str) -> str:
    """Resolve city name/pinyin to Ctrip city ID. If already numeric, return as-is."""
    if city.isdigit():
        return city
    lookup = _load_city_lookup()
    # Try exact match, then lowercase
    if city in lookup:
        return str(lookup[city])
    if city.lower() in lookup:
        return str(lookup[city.lower()])
    logger.warning("City '%s' not found in lookup, using as-is", city)
    return city


async def fetch_hotel_list_page(
    city_id: str,
    checkin: str,
    checkout: str,
    page: int = 1,
) -> str | None:
    """Fetch a single page of Ctrip hotel listings.

    Returns the HTML content or None on failure.
    """
    url = (
        f"https://hotels.ctrip.com/hotels/list"
        f"?countryId=1&city={city_id}"
        f"&checkin={checkin}&checkout={checkout}"
        f"&optionId=1&optionValue=1&direct=0"
        f"&barCur498=&pageNo={page}"
    )
    logger.info("Fetching page %d: %s", page, url)

    try:
        async with httpx.AsyncClient(
            headers=HEADERS,
            follow_redirects=True,
            timeout=30.0,
        ) as client:
            resp = await client.get(url)
            if resp.status_code == 200:
                return resp.text
            logger.warning("Got status %s for page %d", resp.status_code, page)
    except Exception:
        logger.exception("Failed to fetch page %d", page)

    return None


async def fetch_all_pages(
    city: str,
    checkin: str,
    checkout: str,
    max_pages: int = 5,
) -> list[str]:
    """Fetch multiple pages of hotel listings with delays."""
    city_id = resolve_city_id(city)
    pages: list[str] = []

    for page_num in range(1, max_pages + 1):
        html = await fetch_hotel_list_page(city_id, checkin, checkout, page_num)
        if html:
            pages.append(html)
        else:
            logger.info("No more pages after page %d", page_num - 1)
            break

        if page_num < max_pages:
            delay = random.uniform(2.0, 5.0)
            logger.info("Waiting %.1fs before next page", delay)
            await asyncio.sleep(delay)

    return pages
