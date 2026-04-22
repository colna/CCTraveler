from __future__ import annotations

from pydantic import BaseModel


class ScrapeRequest(BaseModel):
    city: str
    checkin: str
    checkout: str
    max_pages: int = 5


class ScrapedRoom(BaseModel):
    name: str
    price: float | None = None
    original_price: float | None = None
    bed_type: str | None = None
    has_breakfast: bool | None = None
    has_free_cancel: bool | None = None


class ScrapedHotel(BaseModel):
    id: str
    name: str
    name_en: str | None = None
    star: int | None = None
    rating: float | None = None
    rating_count: int | None = None
    address: str | None = None
    latitude: float | None = None
    longitude: float | None = None
    image_url: str | None = None
    city: str
    district: str | None = None
    rooms: list[ScrapedRoom] = []


class ScrapeResponse(BaseModel):
    hotels: list[ScrapedHotel]
    total: int
    scraped_at: str
