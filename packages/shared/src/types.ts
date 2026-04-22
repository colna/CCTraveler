export interface Hotel {
  id: string;
  name: string;
  name_en: string | null;
  star: number | null;
  rating: number | null;
  rating_count: number;
  address: string | null;
  city: string;
  district: string | null;
}

export interface HotelWithPrice {
  hotel: Hotel;
  lowest_price: number | null;
  original_price: number | null;
  room_name: string | null;
}

export interface PriceSnapshot {
  id: string;
  room_id: string;
  hotel_id: string;
  price: number;
  original_price: number | null;
  checkin: string;
  checkout: string;
  scraped_at: string;
  source: string;
}
