"""Type definitions for flight scraper."""
from __future__ import annotations

from typing import List, Optional
from pydantic import BaseModel, Field


class FlightCabinPrice(BaseModel):
    """单个舱位的价格信息"""
    cabin_class: str = Field(..., description="舱位等级：头等舱/商务舱/经济舱")
    price: float = Field(..., description="价格（元）")
    discount: Optional[float] = Field(None, description="折扣，如 0.8 表示 8 折")
    available_seats: Optional[int] = Field(None, description="余票数量")


class ScrapedFlight(BaseModel):
    """爬取的机票信息"""
    flight_id: str = Field(..., description="航班号，如 CA1234")
    airline: str = Field(..., description="航空公司")
    from_airport: str = Field(..., description="出发机场代码，如 PEK")
    to_airport: str = Field(..., description="到达机场代码，如 SHA")
    from_city: str = Field(..., description="出发城市")
    to_city: str = Field(..., description="到达城市")
    depart_time: str = Field(..., description="出发时间 HH:MM")
    arrive_time: str = Field(..., description="到达时间 HH:MM")
    duration_minutes: int = Field(..., description="飞行时长（分钟）")
    aircraft_type: Optional[str] = Field(None, description="机型，如 A320")
    source: str = Field("mock", description="数据来源：ctrip/qunar/fliggy/mock 或逗号分隔的多源")
    prices: List[FlightCabinPrice] = Field(default_factory=list, description="舱位价格列表")


class FlightScrapeRequest(BaseModel):
    """机票爬取请求"""
    from_city: str = Field(..., description="出发城市")
    to_city: str = Field(..., description="到达城市")
    travel_date: str = Field(..., description="出行日期 YYYY-MM-DD")


class FlightScrapeResponse(BaseModel):
    """机票爬取响应"""
    flights: List[ScrapedFlight] = Field(default_factory=list)
    total: int = Field(0, description="爬取到的航班总数")
    scraped_at: str = Field(..., description="爬取时间 ISO 8601")
