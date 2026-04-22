from __future__ import annotations

from typing import Optional, List
from pydantic import BaseModel


class ScrapeRequest(BaseModel):
    city: str
    checkin: str
    checkout: str
    max_pages: int = 5
    source: str = "trip"  # "trip" (with prices) or "ctrip" (no prices in SSR)


class ScrapedRoom(BaseModel):
    name: str
    price: Optional[float] = None
    original_price: Optional[float] = None
    currency: str = "CNY"
    bed_type: Optional[str] = None
    has_breakfast: Optional[bool] = None
    has_free_cancel: Optional[bool] = None


class ScrapedHotel(BaseModel):
    id: str
    name: str
    name_en: Optional[str] = None
    star: Optional[int] = None
    rating: Optional[float] = None
    rating_count: Optional[int] = None
    address: Optional[str] = None
    latitude: Optional[float] = None
    longitude: Optional[float] = None
    image_url: Optional[str] = None
    city: str
    district: Optional[str] = None
    rooms: List[ScrapedRoom] = []


class ScrapeResponse(BaseModel):
    hotels: List[ScrapedHotel]
    total: int
    scraped_at: str
