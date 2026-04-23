"""Flight ticket fetcher."""
from __future__ import annotations

import asyncio
import logging
from typing import List

from ..utils.geo_lookup import get_airport_code
from .types import ScrapedFlight, FlightCabinPrice

logger = logging.getLogger(__name__)


async def fetch_flights_ctrip(
    from_city: str,
    to_city: str,
    travel_date: str,
) -> List[ScrapedFlight]:
    """
    从携程爬取机票信息

    注意：这是一个简化的实现框架。实际使用需要：
    1. 使用携程机票 API 或网页爬取
    2. 处理反爬机制
    3. 解析复杂的价格结构（含税、不含税、各种费用）
    4. 处理中转航班
    """
    logger.info(f"Fetching flights from Ctrip: {from_city} -> {to_city} on {travel_date}")

    # TODO: 实际实现携程机票爬取
    # 可以参考现有的 ctrip/fetcher.py 实现

    return []


async def fetch_flights_mock(
    from_city: str,
    to_city: str,
    travel_date: str,
) -> List[ScrapedFlight]:
    """
    Mock 数据用于开发测试
    实际部署时应该使用 fetch_flights_ctrip
    """
    logger.info(f"Using mock data for flights: {from_city} -> {to_city} on {travel_date}")

    # 模拟延迟
    await asyncio.sleep(1)

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
