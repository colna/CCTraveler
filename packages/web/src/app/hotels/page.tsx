import { Suspense } from "react";
import { SearchForm } from "@/components/search-form";
import { HotelCard } from "@/components/hotel-card";
import type { HotelWithPrice } from "@/lib/types";
import Link from "next/link";

async function getHotels(params: Record<string, string>): Promise<{
  hotels: HotelWithPrice[];
  total: number;
  error?: string;
}> {
  const sp = new URLSearchParams(params);
  const baseUrl = process.env.NEXT_PUBLIC_BASE_URL || "http://localhost:3000";
  try {
    const res = await fetch(`${baseUrl}/api/hotels?${sp.toString()}`, {
      cache: "no-store",
    });
    return await res.json();
  } catch {
    return { hotels: [], total: 0, error: "Failed to load hotels" };
  }
}

export default async function HotelsPage({
  searchParams,
}: {
  searchParams: Promise<Record<string, string>>;
}) {
  const params = await searchParams;
  const { hotels, total, error } = await getHotels(params);

  return (
    <main className="min-h-screen">
      <header className="bg-white border-b border-slate-200 px-6 py-4">
        <div className="max-w-6xl mx-auto flex items-center justify-between">
          <Link href="/" className="text-xl font-bold text-slate-900">
            CC<span className="text-amber-500">Traveler</span>
          </Link>
          <span className="text-sm text-slate-400">
            {total} hotels found
          </span>
        </div>
      </header>

      <div className="max-w-6xl mx-auto px-6 py-6">
        <Suspense fallback={null}>
          <SearchForm />
        </Suspense>

        {error && (
          <div className="mt-4 p-3 bg-amber-50 border border-amber-200 rounded-md text-sm text-amber-800">
            {error}
          </div>
        )}

        <div className="mt-6 grid gap-3">
          {hotels.length === 0 && !error ? (
            <div className="text-center py-20 text-slate-400">
              <p className="text-lg">No hotels found</p>
              <p className="text-sm mt-2">
                Run the CLI scraper first:{" "}
                <code className="bg-slate-100 px-2 py-0.5 rounded text-xs">
                  cctraveler scrape --city 遵义 --checkin 2026-05-01 --checkout
                  2026-05-03
                </code>
              </p>
            </div>
          ) : (
            hotels.map((h) => <HotelCard key={h.hotel.id} data={h} />)
          )}
        </div>
      </div>
    </main>
  );
}
