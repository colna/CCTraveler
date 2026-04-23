#!/bin/bash
# 测试 CCTraveler v0.2 交通查询功能

cd /Users/colna/WORK/CCTraveler

echo "=== 测试 1: 查询火车票 ==="
echo "查询5月1日从北京到上海的高铁" | cargo run --package cctraveler -- chat 2>&1 | grep -A 50 "CCTraveler"

echo ""
echo "=== 测试 2: 查询机票 ==="
echo "查询5月1日北京到上海的机票" | cargo run --package cctraveler -- chat 2>&1 | grep -A 50 "CCTraveler"

echo ""
echo "=== 测试 3: 对比交通方案 ==="
echo "对比5月1日北京到上海的交通方案，我比较在意性价比" | cargo run --package cctraveler -- chat 2>&1 | grep -A 50 "CCTraveler"
