"""Ctrip hotel list fetcher using httpx with browser-like headers."""
from __future__ import annotations

import asyncio
import logging
import random

import httpx

logger = logging.getLogger(__name__)

# Known Ctrip city ID mappings
CITY_IDS: dict[str, str] = {
    "zunyi": "558", "遵义": "558",
    "guiyang": "30", "贵阳": "30",
    "chengdu": "28", "成都": "28",
    "chongqing": "4", "重庆": "4",
    "beijing": "1", "北京": "1",
    "shanghai": "2", "上海": "2",
    "guangzhou": "32", "广州": "32",
    "shenzhen": "26", "深圳": "26",
    "hangzhou": "14", "杭州": "14",
    "nanjing": "9", "南京": "9",
    "xian": "7", "西安": "7",
    "kunming": "31", "昆明": "31",
    "dali": "135", "大理": "135",
    "sanya": "43", "三亚": "43",
}

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
    """Resolve city name to Ctrip city ID. If already numeric, return as-is."""
    if city.isdigit():
        return city
    return CITY_IDS.get(city.lower(), CITY_IDS.get(city, city))


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
