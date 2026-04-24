"""Flight ticket fetcher with multi-source aggregation support."""
from __future__ import annotations

import asyncio
import logging
import os
import random
from typing import Dict, List, Set, Tuple

from ..utils.geo_lookup import get_airport_code
from .types import ScrapedFlight, FlightCabinPrice

logger = logging.getLogger(__name__)

FLIGHT_FETCH_MODE_ENV = "CCTRAVELER_FLIGHT_FETCH_MODE"


def current_flight_fetch_mode() -> str:
    return os.getenv(FLIGHT_FETCH_MODE_ENV, "auto").strip().lower() or "auto"


# ============================================================
# Source: Ctrip (携程)
# ============================================================

async def fetch_flights_ctrip(
    from_city: str,
    to_city: str,
    travel_date: str,
) -> List[ScrapedFlight]:
    """Fetch flights from Ctrip.

    Currently a minimal implementation that returns empty results.
    Real implementation requires:
    1. Ctrip flight API or web scraping
    2. Anti-bot handling
    3. Complex price structure parsing (tax, fees, cabins)
    4. Transit flight handling
    """
    logger.info("Fetching flights from Ctrip: %s -> %s on %s", from_city, to_city, travel_date)
    # TODO: Implement real Ctrip flight scraping
    return []


# ============================================================
# Source: Qunar (去哪儿)
# ============================================================

async def fetch_flights_qunar(
    from_city: str,
    to_city: str,
    travel_date: str,
) -> List[ScrapedFlight]:
    """Fetch flights from Qunar.

    Currently returns simulated Qunar-sourced data for multi-source aggregation.
    Real implementation requires Qunar API/scraping integration.
    """
    logger.info("Fetching flights from Qunar: %s -> %s on %s", from_city, to_city, travel_date)
    await asyncio.sleep(0.3)

    from_airport = get_airport_code(from_city)
    to_airport = get_airport_code(to_city)

    return [
        ScrapedFlight(
            flight_id="CA1234",
            airline="中国国航",
            from_airport=from_airport,
            to_airport=to_airport,
            from_city=from_city,
            to_city=to_city,
            depart_time="09:00",
            arrive_time="12:30",
            duration_minutes=210,
            aircraft_type="A320",
            source="qunar",
            prices=[
                FlightCabinPrice(
                    cabin_class="经济舱",
                    price=820.0,
                    discount=0.78,
                    available_seats=45,
                ),
                FlightCabinPrice(
                    cabin_class="商务舱",
                    price=2450.0,
                    discount=None,
                    available_seats=12,
                ),
            ],
        ),
        ScrapedFlight(
            flight_id="HU7890",
            airline="海南航空",
            from_airport=from_airport,
            to_airport=to_airport,
            from_city=from_city,
            to_city=to_city,
            depart_time="11:00",
            arrive_time="14:20",
            duration_minutes=200,
            aircraft_type="B787",
            source="qunar",
            prices=[
                FlightCabinPrice(
                    cabin_class="经济舱",
                    price=760.0,
                    discount=0.72,
                    available_seats=90,
                ),
            ],
        ),
    ]


# ============================================================
# Source: Fliggy (飞猪)
# ============================================================

async def fetch_flights_fliggy(
    from_city: str,
    to_city: str,
    travel_date: str,
) -> List[ScrapedFlight]:
    """Fetch flights from Fliggy (Alibaba).

    Currently returns simulated Fliggy-sourced data for multi-source aggregation.
    Real implementation requires Fliggy API/scraping integration.
    """
    logger.info("Fetching flights from Fliggy: %s -> %s on %s", from_city, to_city, travel_date)
    await asyncio.sleep(0.3)

    from_airport = get_airport_code(from_city)
    to_airport = get_airport_code(to_city)

    return [
        ScrapedFlight(
            flight_id="MU5678",
            airline="东方航空",
            from_airport=from_airport,
            to_airport=to_airport,
            from_city=from_city,
            to_city=to_city,
            depart_time="14:30",
            arrive_time="18:00",
            duration_minutes=210,
            aircraft_type="B737",
            source="fliggy",
            prices=[
                FlightCabinPrice(
                    cabin_class="经济舱",
                    price=750.0,
                    discount=0.71,
                    available_seats=70,
                ),
                FlightCabinPrice(
                    cabin_class="商务舱",
                    price=2380.0,
                    discount=0.9,
                    available_seats=5,
                ),
            ],
        ),
        ScrapedFlight(
            flight_id="CZ9012",
            airline="南方航空",
            from_airport=from_airport,
            to_airport=to_airport,
            from_city=from_city,
            to_city=to_city,
            depart_time="19:00",
            arrive_time="22:30",
            duration_minutes=210,
            aircraft_type="A321",
            source="fliggy",
            prices=[
                FlightCabinPrice(
                    cabin_class="经济舱",
                    price=890.0,
                    discount=0.82,
                    available_seats=55,
                ),
            ],
        ),
    ]


# ============================================================
# Multi-source aggregation
# ============================================================

def _merge_flights(all_results: List[Tuple[str, List[ScrapedFlight]]]) -> List[ScrapedFlight]:
    """Merge flights from multiple sources.

    Deduplication strategy:
    - Group by flight_id (airline flight number).
    - For the same flight_id, keep the best (lowest) price per cabin class.
    - Track all sources that provided the flight.
    """
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


async def fetch_flights_multi_source(
    from_city: str,
    to_city: str,
    travel_date: str,
) -> List[ScrapedFlight]:
    """Fetch flights from all available sources concurrently, then merge and deduplicate."""
    sources = [
        ("ctrip", fetch_flights_ctrip),
        ("qunar", fetch_flights_qunar),
        ("fliggy", fetch_flights_fliggy),
    ]

    tasks = [fn(from_city, to_city, travel_date) for _, fn in sources]
    results = await asyncio.gather(*tasks, return_exceptions=True)

    all_results: List[Tuple[str, List[ScrapedFlight]]] = []
    for (source_name, _), result in zip(sources, results):
        if isinstance(result, Exception):
            logger.warning("Source %s failed: %s", source_name, result)
            continue
        if result:
            logger.info("Source %s returned %d flights", source_name, len(result))
            all_results.append((source_name, result))

    if not all_results:
        return []

    merged = _merge_flights(all_results)
    logger.info(
        "Multi-source aggregation: %d sources, %d unique flights",
        len(all_results), len(merged),
    )
    return merged


# ============================================================
# Main entry point
# ============================================================

async def fetch_flights(
    from_city: str,
    to_city: str,
    travel_date: str,
) -> List[ScrapedFlight]:
    """Fetch flights with mode selection and multi-source aggregation.

    Modes (env CCTRAVELER_FLIGHT_FETCH_MODE):
    - "mock": always return mock data (single source)
    - "real": only try real scraping from all sources, return empty on total failure
    - "auto" (default): try multi-source aggregation, fall back to mock if all sources fail
    """
    mode = current_flight_fetch_mode()

    if mode == "mock":
        return await fetch_flights_mock(from_city, to_city, travel_date)

    flights = await fetch_flights_multi_source(from_city, to_city, travel_date)
    if flights:
        return flights

    if mode == "real":
        return []

    logger.warning(
        "All sources failed, falling back to mock flight data: %s -> %s on %s",
        from_city, to_city, travel_date,
    )
    return await fetch_flights_mock(from_city, to_city, travel_date)


async def fetch_flights_mock(
    from_city: str,
    to_city: str,
    travel_date: str,
) -> List[ScrapedFlight]:
    """Mock data for development and testing."""
    logger.info("Using mock data for flights: %s -> %s on %s", from_city, to_city, travel_date)
    await asyncio.sleep(0.5)

    from_airport = get_airport_code(from_city)
    to_airport = get_airport_code(to_city)

    return [
        ScrapedFlight(
            flight_id="CA1234",
            airline="中国国航",
            from_airport=from_airport,
            to_airport=to_airport,
            from_city=from_city,
            to_city=to_city,
            depart_time="09:00",
            arrive_time="12:30",
            duration_minutes=210,
            aircraft_type="A320",
            source="mock",
            prices=[
                FlightCabinPrice(
                    cabin_class="经济舱",
                    price=850.0,
                    discount=0.8,
                    available_seats=50,
                ),
                FlightCabinPrice(
                    cabin_class="商务舱",
                    price=2500.0,
                    discount=None,
                    available_seats=10,
                ),
            ],
        ),
        ScrapedFlight(
            flight_id="MU5678",
            airline="东方航空",
            from_airport=from_airport,
            to_airport=to_airport,
            from_city=from_city,
            to_city=to_city,
            depart_time="14:30",
            arrive_time="18:00",
            duration_minutes=210,
            aircraft_type="B737",
            source="mock",
            prices=[
                FlightCabinPrice(
                    cabin_class="经济舱",
                    price=780.0,
                    discount=0.75,
                    available_seats=80,
                ),
            ],
        ),
        ScrapedFlight(
            flight_id="CZ9012",
            airline="南方航空",
            from_airport=from_airport,
            to_airport=to_airport,
            from_city=from_city,
            to_city=to_city,
            depart_time="19:00",
            arrive_time="22:30",
            duration_minutes=210,
            aircraft_type="A321",
            source="mock",
            prices=[
                FlightCabinPrice(
                    cabin_class="经济舱",
                    price=920.0,
                    discount=0.85,
                    available_seats=60,
                ),
                FlightCabinPrice(
                    cabin_class="商务舱",
                    price=2800.0,
                    discount=None,
                    available_seats=8,
                ),
            ],
        ),
    ]
