"""FastAPI scraper service for CCTraveler."""
from __future__ import annotations

import logging
from datetime import datetime, timezone
from typing import Optional, Set

from fastapi import FastAPI, HTTPException

from .ctrip.fetcher import fetch_all_pages, resolve_city_id, _load_city_lookup
from .ctrip.parser import parse_hotel_list
from .ctrip.types import ScrapeRequest, ScrapeResponse
from .train.types import TrainScrapeRequest, TrainScrapeResponse
from .train.fetcher import fetch_trains_mock
from .flight.types import FlightScrapeRequest, FlightScrapeResponse
from .flight.fetcher import fetch_flights_mock

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

app = FastAPI(title="CCTraveler Scraper", version="0.2.0")


@app.get("/health")
async def health():
    return {"status": "ok", "service": "cctraveler-scraper"}


@app.post("/scrape/hotels", response_model=ScrapeResponse)
async def scrape_hotels(req: ScrapeRequest):
    """Scrape hotel listings. Uses Trip.com (with prices) by default."""
    source = req.source
    logger.info(
        "Scraping hotels: city=%s, %s to %s, max_pages=%d, source=%s",
        req.city, req.checkin, req.checkout, req.max_pages, source,
    )

    try:
        pages = await fetch_all_pages(
            city=req.city,
            checkin=req.checkin,
            checkout=req.checkout,
            max_pages=req.max_pages,
            source=source,
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
    seen_ids: Set[str] = set()

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
async def list_cities(q: Optional[str] = None):
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


@app.post("/scrape/trains", response_model=TrainScrapeResponse)
async def scrape_trains(req: TrainScrapeRequest):
    """Scrape train tickets from 12306 (currently using mock data)."""
    logger.info(
        "Scraping trains: %s -> %s on %s",
        req.from_city, req.to_city, req.travel_date,
    )

    try:
        # TODO: 切换到 fetch_trains_12306 当反爬机制完善后
        trains = await fetch_trains_mock(
            from_city=req.from_city,
            to_city=req.to_city,
            travel_date=req.travel_date,
        )
    except Exception as e:
        logger.exception("Failed to fetch trains")
        raise HTTPException(status_code=502, detail=f"Scraping failed: {e}") from e

    logger.info("Scraped %d trains", len(trains))

    return TrainScrapeResponse(
        trains=trains,
        total=len(trains),
        scraped_at=datetime.now(timezone.utc).isoformat(),
    )


@app.post("/scrape/flights", response_model=FlightScrapeResponse)
async def scrape_flights(req: FlightScrapeRequest):
    """Scrape flight tickets (currently using mock data)."""
    logger.info(
        "Scraping flights: %s -> %s on %s",
        req.from_city, req.to_city, req.travel_date,
    )

    try:
        # TODO: 切换到 fetch_flights_ctrip 当实现完成后
        flights = await fetch_flights_mock(
            from_city=req.from_city,
            to_city=req.to_city,
            travel_date=req.travel_date,
        )
    except Exception as e:
        logger.exception("Failed to fetch flights")
        raise HTTPException(status_code=502, detail=f"Scraping failed: {e}") from e

    logger.info("Scraped %d flights", len(flights))

    return FlightScrapeResponse(
        flights=flights,
        total=len(flights),
        scraped_at=datetime.now(timezone.utc).isoformat(),
    )


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(app, host="0.0.0.0", port=8300)
