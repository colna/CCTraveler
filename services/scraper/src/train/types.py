"""Type definitions for train scraper."""
from __future__ import annotations

from typing import List, Optional
from pydantic import BaseModel, Field


class TrainSeatPrice(BaseModel):
    """单个座位类型的价格信息"""
    seat_type: str = Field(..., description="座位类型：商务座/一等座/二等座/硬卧/软卧")
    price: float = Field(..., description="价格（元）")
    available_seats: Optional[int] = Field(None, description="余票数量，-1表示未知")


class ScrapedTrain(BaseModel):
    """爬取的火车票信息"""
    train_id: str = Field(..., description="车次号，如 G1234")
    train_type: str = Field(..., description="车型：G/D/C/K/T/Z")
    from_station: str = Field(..., description="出发站")
    to_station: str = Field(..., description="到达站")
    from_city: str = Field(..., description="出发城市")
    to_city: str = Field(..., description="到达城市")
    depart_time: str = Field(..., description="出发时间 HH:MM")
    arrive_time: str = Field(..., description="到达时间 HH:MM")
    duration_minutes: int = Field(..., description="运行时长（分钟）")
    distance_km: Optional[int] = Field(None, description="里程（公里）")
    seats: List[TrainSeatPrice] = Field(default_factory=list, description="座位价格列表")


class TrainScrapeRequest(BaseModel):
    """火车票爬取请求"""
    from_city: str = Field(..., description="出发城市")
    to_city: str = Field(..., description="到达城市")
    travel_date: str = Field(..., description="出行日期 YYYY-MM-DD")


class TrainScrapeResponse(BaseModel):
    """火车票爬取响应"""
    trains: List[ScrapedTrain] = Field(default_factory=list)
    total: int = Field(0, description="爬取到的车次总数")
    scraped_at: str = Field(..., description="爬取时间 ISO 8601")
