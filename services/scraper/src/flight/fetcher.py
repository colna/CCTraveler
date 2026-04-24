"""Flight ticket fetcher with auto/mock/real mode support."""
from __future__ import annotations

import asyncio
import logging
import os
from typing import List

from ..utils.geo_lookup import get_airport_code
from .types import ScrapedFlight, FlightCabinPrice

logger = logging.getLogger(__name__)

FLIGHT_FETCH_MODE_ENV = "CCTRAVELER_FLIGHT_FETCH_MODE"


def current_flight_fetch_mode() -> str:
    return os.getenv(FLIGHT_FETCH_MODE_ENV, "auto").strip().lower() or "auto"


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


async def fetch_flights(
    from_city: str,
    to_city: str,
    travel_date: str,
) -> List[ScrapedFlight]:
    """Fetch flights with mode selection (mirrors train's fetch_trains pattern).

    Modes (env CCTRAVELER_FLIGHT_FETCH_MODE):
    - "mock": always return mock data
    - "real": only try real scraping, return empty on failure
    - "auto" (default): try real first, fall back to mock
    """
    mode = current_flight_fetch_mode()

    if mode == "mock":
        return await fetch_flights_mock(from_city, to_city, travel_date)

    flights = await fetch_flights_ctrip(from_city, to_city, travel_date)
    if flights:
        return flights

    if mode == "real":
        return []

    logger.warning(
        "Falling back to mock flight data: %s -> %s on %s",
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
