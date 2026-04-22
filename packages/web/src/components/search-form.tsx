"use client";

import { useRouter, useSearchParams } from "next/navigation";
import { useState } from "react";

export function SearchForm() {
  const router = useRouter();
  const params = useSearchParams();

  const [city, setCity] = useState(params.get("city") || "");
  const [maxPrice, setMaxPrice] = useState(params.get("max_price") || "");
  const [minStar, setMinStar] = useState(params.get("min_star") || "");
  const [sortBy, setSortBy] = useState(params.get("sort_by") || "price");

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    const sp = new URLSearchParams();
    if (city) sp.set("city", city);
    if (maxPrice) sp.set("max_price", maxPrice);
    if (minStar) sp.set("min_star", minStar);
    if (sortBy) sp.set("sort_by", sortBy);
    router.push(`/hotels?${sp.toString()}`);
  }

  return (
    <form onSubmit={handleSubmit} className="flex flex-wrap items-end gap-3">
      <div>
        <label className="block text-xs font-medium text-slate-500 mb-1">
          城市
        </label>
        <input
          type="text"
          value={city}
          onChange={(e) => setCity(e.target.value)}
          placeholder="遵义"
          className="px-3 py-2 border border-slate-300 rounded-md text-sm w-32 focus:outline-none focus:ring-2 focus:ring-amber-400 focus:border-transparent"
        />
      </div>

      <div>
        <label className="block text-xs font-medium text-slate-500 mb-1">
          最高价格
        </label>
        <input
          type="number"
          value={maxPrice}
          onChange={(e) => setMaxPrice(e.target.value)}
          placeholder="500"
          className="px-3 py-2 border border-slate-300 rounded-md text-sm w-24 focus:outline-none focus:ring-2 focus:ring-amber-400 focus:border-transparent"
        />
      </div>

      <div>
        <label className="block text-xs font-medium text-slate-500 mb-1">
          最低星级
        </label>
        <select
          value={minStar}
          onChange={(e) => setMinStar(e.target.value)}
          className="px-3 py-2 border border-slate-300 rounded-md text-sm focus:outline-none focus:ring-2 focus:ring-amber-400 focus:border-transparent"
        >
          <option value="">不限</option>
          <option value="3">3星+</option>
          <option value="4">4星+</option>
          <option value="5">5星</option>
        </select>
      </div>

      <div>
        <label className="block text-xs font-medium text-slate-500 mb-1">
          排序
        </label>
        <select
          value={sortBy}
          onChange={(e) => setSortBy(e.target.value)}
          className="px-3 py-2 border border-slate-300 rounded-md text-sm focus:outline-none focus:ring-2 focus:ring-amber-400 focus:border-transparent"
        >
          <option value="price">价格</option>
          <option value="rating">评分</option>
          <option value="star">星级</option>
        </select>
      </div>

      <button
        type="submit"
        className="px-5 py-2 bg-slate-900 text-white text-sm font-medium rounded-md hover:bg-amber-600 transition-colors"
      >
        搜索
      </button>
    </form>
  );
}
