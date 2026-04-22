import { NextRequest, NextResponse } from "next/server";
import { getDb } from "@/lib/db";

export async function GET(request: NextRequest) {
  const params = request.nextUrl.searchParams;
  const city = params.get("city");
  const maxPrice = params.get("max_price");
  const minStar = params.get("min_star");
  const sortBy = params.get("sort_by") || "price";
  const limit = parseInt(params.get("limit") || "50", 10);

  try {
    const db = getDb();

    let sql = `
      SELECT h.*, MIN(p.price) as lowest_price, p.original_price, r.name as room_name
      FROM hotels h
      LEFT JOIN price_snapshots p ON h.id = p.hotel_id
      LEFT JOIN rooms r ON p.room_id = r.id
      WHERE 1=1
    `;
    const sqlParams: (string | number)[] = [];

    if (city) {
      sql += " AND h.city = ?";
      sqlParams.push(city);
    }
    if (minStar) {
      sql += " AND h.star >= ?";
      sqlParams.push(parseInt(minStar, 10));
    }

    sql += " GROUP BY h.id";

    if (maxPrice) {
      sql += ` HAVING lowest_price <= ?`;
      sqlParams.push(parseFloat(maxPrice));
    }

    const orderMap: Record<string, string> = {
      price: "lowest_price ASC",
      rating: "h.rating DESC",
      star: "h.star DESC",
    };
    sql += ` ORDER BY ${orderMap[sortBy] || "lowest_price ASC"}`;
    sql += ` LIMIT ?`;
    sqlParams.push(limit);

    const rows = db.prepare(sql).all(...sqlParams);

    const hotels = rows.map((row: Record<string, unknown>) => ({
      hotel: {
        id: row.id,
        name: row.name,
        name_en: row.name_en,
        star: row.star,
        rating: row.rating,
        rating_count: row.rating_count || 0,
        address: row.address,
        latitude: row.latitude,
        longitude: row.longitude,
        image_url: row.image_url,
        amenities: JSON.parse((row.amenities as string) || "[]"),
        city: row.city,
        district: row.district,
        created_at: row.created_at,
        updated_at: row.updated_at,
      },
      lowest_price: row.lowest_price,
      original_price: row.original_price,
      room_name: row.room_name,
    }));

    return NextResponse.json({ hotels, total: hotels.length });
  } catch (error) {
    const message =
      error instanceof Error ? error.message : "Database error";
    return NextResponse.json({ hotels: [], total: 0, error: message });
  }
}
