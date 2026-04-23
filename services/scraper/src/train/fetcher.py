"""12306 train ticket fetcher using Selenium."""
from __future__ import annotations

import asyncio
import logging
import re
from datetime import datetime
from typing import List

from selenium import webdriver
from selenium.webdriver.common.by import By
from selenium.webdriver.support.ui import WebDriverWait
from selenium.webdriver.support import expected_conditions as EC
from selenium.common.exceptions import TimeoutException, NoSuchElementException

from ..utils.geo_lookup import get_station_code
from .types import ScrapedTrain, TrainSeatPrice

logger = logging.getLogger(__name__)


def parse_train_type(train_id: str) -> str:
    """从车次号解析车型"""
    if train_id.startswith("G"):
        return "G"
    elif train_id.startswith("D"):
        return "D"
    elif train_id.startswith("C"):
        return "C"
    elif train_id.startswith("K"):
        return "K"
    elif train_id.startswith("T"):
        return "T"
    elif train_id.startswith("Z"):
        return "Z"
    else:
        return "其他"


def parse_duration(duration_str: str) -> int:
    """解析时长字符串为分钟数

    Examples:
        "08:30" -> 510
        "1小时30分" -> 90
    """
    # 格式1: HH:MM
    if ":" in duration_str:
        parts = duration_str.split(":")
        return int(parts[0]) * 60 + int(parts[1])

    # 格式2: X小时Y分
    hours = 0
    minutes = 0
    hour_match = re.search(r"(\d+)小时", duration_str)
    min_match = re.search(r"(\d+)分", duration_str)

    if hour_match:
        hours = int(hour_match.group(1))
    if min_match:
        minutes = int(min_match.group(1))

    return hours * 60 + minutes


async def fetch_trains_12306(
    from_city: str,
    to_city: str,
    travel_date: str,
) -> List[ScrapedTrain]:
    """
    从 12306 爬取火车票信息

    注意：这是一个简化的实现框架。实际使用需要：
    1. 使用 undetected-chromedriver 绕过反爬
    2. 处理验证码（OCR 或人工打码）
    3. 添加随机延迟和鼠标轨迹模拟
    4. 完善车站代码映射表
    """
    from_code = get_station_code(from_city)
    to_code = get_station_code(to_city)

    if not from_code or not to_code:
        logger.error(f"无法找到车站代码: {from_city} -> {to_city}")
        return []

    # 格式化日期为 12306 格式 (YYYY-MM-DD)
    date_obj = datetime.strptime(travel_date, "%Y-%m-%d")
    formatted_date = date_obj.strftime("%Y-%m-%d")

    url = f"https://kyfw.12306.cn/otn/leftTicket/init?linktypeid=dc&fs={from_code}&ts={to_code}&date={formatted_date}&flag=N,N,Y"

    logger.info(f"Fetching trains from 12306: {url}")

    # TODO: 实际实现需要使用 undetected-chromedriver
    # 这里提供一个框架示例

    options = webdriver.ChromeOptions()
    options.add_argument("--headless")
    options.add_argument("--no-sandbox")
    options.add_argument("--disable-dev-shm-usage")

    driver = None
    trains = []

    try:
        driver = webdriver.Chrome(options=options)
        driver.get(url)

        # 等待列表加载
        wait = WebDriverWait(driver, 10)
        wait.until(EC.presence_of_element_located((By.ID, "queryLeftTable")))

        # 解析车次列表
        # 注意：12306 的 DOM 结构可能变化，需要实际调试
        train_rows = driver.find_elements(By.CSS_SELECTOR, "#queryLeftTable tbody tr")

        for row in train_rows:
            try:
                # 提取车次信息（示例，实际需要根据 DOM 调整）
                train_id = row.find_element(By.CSS_SELECTOR, ".train .number").text.strip()
                from_station = row.find_element(By.CSS_SELECTOR, ".cds .start-t").text.strip()
                to_station = row.find_element(By.CSS_SELECTOR, ".cds .color999").text.strip()
                depart_time = row.find_element(By.CSS_SELECTOR, ".cds .start-t").text.strip()
                arrive_time = row.find_element(By.CSS_SELECTOR, ".cds .color999").text.strip()
                duration = row.find_element(By.CSS_SELECTOR, ".ls strong").text.strip()

                # 提取座位价格
                seats = []
                seat_elements = row.find_elements(By.CSS_SELECTOR, ".ticket")

                # 这里需要根据实际 DOM 结构解析各个座位类型和价格
                # 示例：二等座、一等座、商务座等

                train = ScrapedTrain(
                    train_id=train_id,
                    train_type=parse_train_type(train_id),
                    from_station=from_station,
                    to_station=to_station,
                    from_city=from_city,
                    to_city=to_city,
                    depart_time=depart_time,
                    arrive_time=arrive_time,
                    duration_minutes=parse_duration(duration),
                    distance_km=None,
                    seats=seats,
                )

                trains.append(train)

            except (NoSuchElementException, ValueError) as e:
                logger.warning(f"Failed to parse train row: {e}")
                continue

        logger.info(f"Fetched {len(trains)} trains from 12306")

    except TimeoutException:
        logger.error("Timeout waiting for 12306 page to load")
    except Exception as e:
        logger.exception(f"Error fetching trains from 12306: {e}")
    finally:
        if driver:
            driver.quit()

    return trains


async def fetch_trains_mock(
    from_city: str,
    to_city: str,
    travel_date: str,
) -> List[ScrapedTrain]:
    """
    Mock 数据用于开发测试
    实际部署时应该使用 fetch_trains_12306
    """
    logger.info(f"Using mock data for {from_city} -> {to_city} on {travel_date}")

    # 模拟延迟
    await asyncio.sleep(1)

    return [
        ScrapedTrain(
            train_id="G1234",
            train_type="G",
            from_station=f"{from_city}西",
            to_station=to_city,
            from_city=from_city,
            to_city=to_city,
            depart_time="08:00",
            arrive_time="16:30",
            duration_minutes=510,
            distance_km=1800,
            seats=[
                TrainSeatPrice(seat_type="二等座", price=650.5, available_seats=99),
                TrainSeatPrice(seat_type="一等座", price=1040.0, available_seats=15),
                TrainSeatPrice(seat_type="商务座", price=1950.0, available_seats=5),
            ],
        ),
        ScrapedTrain(
            train_id="D5678",
            train_type="D",
            from_station=from_city,
            to_station=to_city,
            from_city=from_city,
            to_city=to_city,
            depart_time="10:30",
            arrive_time="20:15",
            duration_minutes=585,
            distance_km=1800,
            seats=[
                TrainSeatPrice(seat_type="二等座", price=550.0, available_seats=120),
                TrainSeatPrice(seat_type="一等座", price=880.0, available_seats=30),
            ],
        ),
    ]
