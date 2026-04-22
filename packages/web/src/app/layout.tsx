import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "CCTraveler — AI Travel Planner",
  description: "AI-powered hotel price intelligence and travel planning",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="zh-CN">
      <body className="min-h-screen bg-slate-50 antialiased">{children}</body>
    </html>
  );
}
