#!/usr/bin/env python3
"""测试城市地理数据查询"""
import sqlite3
import json

db_path = "/Users/colna/WORK/CCTraveler/data/cctraveler.db"
conn = sqlite3.connect(db_path)

print("=== 测试 1: 查询遵义概况 ===")
cursor = conn.execute("""
    SELECT name, province, tier, population, area_km2, description
    FROM cities WHERE name = '遵义'
""")
row = cursor.fetchone()
if row:
    print(f"城市: {row[0]}")
    print(f"省份: {row[1]}")
    print(f"等级: {row[2]}线城市")
    print(f"人口: {row[3]:,}")
    print(f"面积: {row[4]} km²")
    print(f"简介: {row[5]}")
else:
    print("未找到数据")

print("\n=== 测试 2: 查询遵义区域 ===")
cursor = conn.execute("""
    SELECT d.name, d.description, d.tags
    FROM districts d
    JOIN cities c ON d.city_id = c.id
    WHERE c.name = '遵义'
""")
for row in cursor:
    tags = json.loads(row[2])
    print(f"- {row[0]}: {row[1]} (标签: {', '.join(tags)})")

print("\n=== 测试 3: 查询遵义景点 ===")
cursor = conn.execute("""
    SELECT a.name, a.category, a.rating, a.ticket_price, a.visit_duration_hours
    FROM attractions a
    JOIN cities c ON a.city_id = c.id
    WHERE c.name = '遵义'
""")
for row in cursor:
    print(f"- {row[0]} ({row[1]}) - 评分: {row[2]}, 门票: ¥{row[3]}, 游玩时长: {row[4]}小时")

print("\n=== 测试 4: 查询遵义交通 ===")
print("火车站:")
cursor = conn.execute("SELECT station_name, station_code FROM station_codes WHERE city = '遵义'")
for row in cursor:
    print(f"  - {row[0]} ({row[1]})")

print("机场:")
cursor = conn.execute("SELECT airport_name, airport_code FROM airport_codes WHERE city = '遵义'")
for row in cursor:
    print(f"  - {row[0]} ({row[1]})")

print("\n=== 测试 5: 查询北京景点 ===")
cursor = conn.execute("""
    SELECT a.name, a.category, a.rating, a.ticket_price
    FROM attractions a
    JOIN cities c ON a.city_id = c.id
    WHERE c.name = '北京'
    LIMIT 3
""")
for row in cursor:
    print(f"- {row[0]} ({row[1]}) - 评分: {row[2]}, 门票: ¥{row[3]}")

conn.close()
print("\n✅ 所有测试完成！")
