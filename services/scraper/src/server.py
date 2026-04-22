"""FastAPI scraper service for CCTraveler."""
from __future__ import annotations

import logging
from datetime import datetime, timezone

from fastapi import FastAPI, HTTPException

from .ctrip.fetcher import fetch_all_pages, resolve_city_id, _load_city_lookup
from .ctrip.parser import parse_hotel_list
from .ctrip.types import ScrapeRequest, ScrapeResponse

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

app = FastAPI(title="CCTraveler Scraper", version="0.1.0")


@app.get("/health")
async def health():
    return {"status": "ok", "service": "cctraveler-scraper"}


@app.post("/scrape/hotels", response_model=ScrapeResponse)
async def scrape_hotels(req: ScrapeRequest):
    """Scrape hotel listings from Ctrip for given city and dates."""
    logger.info(
        "Scraping hotels: city=%s, %s to %s, max_pages=%d",
        req.city, req.checkin, req.checkout, req.max_pages,
    )

    try:
        pages = await fetch_all_pages(
            city=req.city,
            checkin=req.checkin,
            checkout=req.checkout,
            max_pages=req.max_pages,
        )
    except Exception as e:
        logger.exception("Failed to fetch pages")
        raise HTTPException(status_code=502, detail=f"Scraping failed: {e}") from e

    if not pages:
        return ScrapeResponse(
            hotels=[],
            total=0,
            scraped_at=datetime.now(timezone.utc).isoformat(),
        )

    all_hotels = []
    seen_ids: set[str] = set()

    for html in pages:
        hotels = parse_hotel_list(html, city=req.city)
        for hotel in hotels:
            if hotel.id not in seen_ids:
                seen_ids.add(hotel.id)
                all_hotels.append(hotel)

    logger.info("Scraped %d unique hotels from %d pages", len(all_hotels), len(pages))

    return ScrapeResponse(
        hotels=all_hotels,
        total=len(all_hotels),
        scraped_at=datetime.now(timezone.utc).isoformat(),
    )


@app.get("/cities")
async def list_cities(q: str | None = None):
    """List supported cities. Optionally filter by query string."""
    lookup = _load_city_lookup()
    # Build deduplicated list from the full cities JSON
    from pathlib import Path
    import json
    cities_path = Path(__file__).resolve().parents[3] / "data" / "ctrip_cities.json"
    if not cities_path.exists():
        return {"total": 0, "cities": []}
    with open(cities_path, encoding="utf-8") as f:
        data = json.load(f)
    cities = data.get("cities", [])
    if q:
        q_lower = q.lower()
        cities = [c for c in cities if q_lower in c["name"] or q_lower in c["pinyin"]]
    return {"total": len(cities), "cities": cities}


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(app, host="0.0.0.0", port=8300)
