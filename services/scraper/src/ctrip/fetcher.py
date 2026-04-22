"""Ctrip hotel list fetcher using Scrapling's StealthyFetcher."""
from __future__ import annotations

import asyncio
import logging
import random

from scrapling import StealthyFetcher

logger = logging.getLogger(__name__)

# Known Ctrip city ID mappings
CITY_IDS: dict[str, str] = {
    "遵义": "558",
    "贵阳": "30",
    "成都": "28",
    "重庆": "4",
    "北京": "1",
    "上海": "2",
    "广州": "32",
    "深圳": "26",
    "杭州": "14",
    "南京": "9",
    "西安": "7",
    "昆明": "31",
    "大理": "135",
    "三亚": "43",
}


def resolve_city_id(city: str) -> str:
    """Resolve city name to Ctrip city ID. If already numeric, return as-is."""
    if city.isdigit():
        return city
    return CITY_IDS.get(city, city)


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
        fetcher = StealthyFetcher()
        resp = await asyncio.to_thread(
            fetcher.fetch,
            url,
            headless=True,
            block_webrtc=True,
            hide_canvas=True,
        )
        if resp and resp.status == 200:
            return resp.html_content
        logger.warning("Got status %s for page %d", resp.status if resp else "None", page)
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
