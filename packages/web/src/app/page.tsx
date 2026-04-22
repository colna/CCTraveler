import Link from "next/link";

export default function Home() {
  return (
    <main className="min-h-screen flex flex-col items-center justify-center px-6">
      <div className="max-w-2xl w-full text-center">
        <h1 className="text-5xl font-bold tracking-tight text-slate-900">
          CC<span className="text-amber-500">Traveler</span>
        </h1>
        <p className="mt-4 text-lg text-slate-500">
          AI Travel Planner — Find the best hotels at the best prices.
        </p>

        <div className="mt-10">
          <Link
            href="/hotels"
            className="inline-block px-8 py-3 bg-slate-900 text-white font-medium rounded-lg hover:bg-amber-600 transition-colors text-lg"
          >
            Browse Hotels
          </Link>
        </div>

        <div className="mt-16 grid grid-cols-3 gap-6 text-left">
          <div className="p-4 bg-white rounded-lg border border-slate-200">
            <p className="font-semibold text-slate-900">Price Intelligence</p>
            <p className="mt-1 text-sm text-slate-500">
              Track hotel prices over time. Know when to book.
            </p>
          </div>
          <div className="p-4 bg-white rounded-lg border border-slate-200">
            <p className="font-semibold text-slate-900">Smart Filtering</p>
            <p className="mt-1 text-sm text-slate-500">
              Filter by price, star rating, user reviews.
            </p>
          </div>
          <div className="p-4 bg-white rounded-lg border border-slate-200">
            <p className="font-semibold text-slate-900">Data Export</p>
            <p className="mt-1 text-sm text-slate-500">
              Export hotel data as CSV or JSON for analysis.
            </p>
          </div>
        </div>
      </div>
    </main>
  );
}
