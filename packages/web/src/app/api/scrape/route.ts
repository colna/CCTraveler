import { NextRequest, NextResponse } from "next/server";

const SCRAPER_URL = process.env.SCRAPER_URL || "http://localhost:8300";

export async function POST(request: NextRequest) {
  try {
    const body = await request.json();
    const resp = await fetch(`${SCRAPER_URL}/scrape/hotels`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });

    if (!resp.ok) {
      const text = await resp.text();
      return NextResponse.json(
        { error: `Scraper error: ${text}` },
        { status: resp.status },
      );
    }

    const data = await resp.json();
    return NextResponse.json(data);
  } catch (error) {
    const message =
      error instanceof Error ? error.message : "Scraper unavailable";
    return NextResponse.json({ error: message }, { status: 502 });
  }
}
