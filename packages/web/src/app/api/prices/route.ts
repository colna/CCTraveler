import { NextRequest, NextResponse } from "next/server";
import { getDb } from "@/lib/db";

export async function GET(request: NextRequest) {
  const hotelId = request.nextUrl.searchParams.get("hotel_id");
  if (!hotelId) {
    return NextResponse.json(
      { error: "hotel_id is required" },
      { status: 400 },
    );
  }

  try {
    const db = getDb();
    const rows = db
      .prepare(
        "SELECT * FROM price_snapshots WHERE hotel_id = ? ORDER BY scraped_at DESC",
      )
      .all(hotelId);
    return NextResponse.json({ prices: rows });
  } catch (error) {
    const message =
      error instanceof Error ? error.message : "Database error";
    return NextResponse.json({ prices: [], error: message });
  }
}
