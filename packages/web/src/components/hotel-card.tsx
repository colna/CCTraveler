"use client";

import type { HotelWithPrice } from "@/lib/types";

function StarDisplay({ count }: { count: number | null }) {
  if (!count) return null;
  return (
    <span className="text-amber-500 text-sm tracking-tight">
      {"★".repeat(count)}
    </span>
  );
}

export function HotelCard({ data }: { data: HotelWithPrice }) {
  const { hotel, lowest_price, original_price, room_name } = data;
  const hasDiscount = original_price && lowest_price && original_price > lowest_price;
  const discountPct = hasDiscount
    ? Math.round((1 - lowest_price / original_price) * 100)
    : 0;

  return (
    <div className="group relative bg-white border border-slate-200 rounded-lg p-5 hover:shadow-lg hover:border-amber-300 transition-all duration-200">
      {hasDiscount && discountPct > 0 && (
        <span className="absolute -top-2 -right-2 bg-red-500 text-white text-xs font-bold px-2 py-0.5 rounded-full">
          -{discountPct}%
        </span>
      )}

      <div className="flex justify-between items-start gap-4">
        <div className="flex-1 min-w-0">
          <h3 className="font-semibold text-lg text-slate-900 truncate">
            {hotel.name}
          </h3>
          <div className="mt-1 flex items-center gap-2">
            <StarDisplay count={hotel.star} />
            {hotel.rating && (
              <span className="text-sm font-medium text-slate-600">
                {hotel.rating.toFixed(1)}分
                {hotel.rating_count > 0 && (
                  <span className="text-slate-400 ml-1">
                    ({hotel.rating_count.toLocaleString()}评)
                  </span>
                )}
              </span>
            )}
          </div>
          {hotel.address && (
            <p className="mt-1.5 text-sm text-slate-500 truncate">
              {hotel.district && <span className="font-medium">{hotel.district} · </span>}
              {hotel.address}
            </p>
          )}
          {room_name && (
            <p className="mt-1 text-xs text-slate-400">{room_name}</p>
          )}
        </div>

        <div className="text-right shrink-0">
          {lowest_price ? (
            <>
              <p className="text-2xl font-bold text-amber-600">
                ¥{Math.round(lowest_price)}
              </p>
              <p className="text-xs text-slate-400">/晚</p>
              {hasDiscount && (
                <p className="text-xs text-slate-400 line-through">
                  ¥{Math.round(original_price)}
                </p>
              )}
            </>
          ) : (
            <p className="text-sm text-slate-400">暂无价格</p>
          )}
        </div>
      </div>
    </div>
  );
}
