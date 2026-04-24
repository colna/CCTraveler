"""12306 train ticket fetcher using Selenium."""
from __future__ import annotations

import asyncio
import logging
import os
import re
from datetime import datetime
from typing import List, Optional

from selenium import webdriver
from selenium.webdriver.common.by import By
from selenium.webdriver.support.ui import WebDriverWait
from selenium.webdriver.support import expected_conditions as EC
from selenium.common.exceptions import TimeoutException, NoSuchElementException, WebDriverException

from ..utils.geo_lookup import get_station_code
from .types import ScrapedTrain, TrainSeatPrice

logger = logging.getLogger(__name__)

TRAIN_FETCH_MODE_ENV = "CCTRAVELER_TRAIN_FETCH_MODE"


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


def current_train_fetch_mode() -> str:
    return os.getenv(TRAIN_FETCH_MODE_ENV, "auto").strip().lower() or "auto"


def extract_time_parts(raw_text: str) -> tuple[Optional[str], Optional[str]]:
    matches = re.findall(r"\b\d{2}:\d{2}\b", raw_text)
    if len(matches) >= 2:
        return matches[0], matches[1]
    if len(matches) == 1:
        return matches[0], None
    return None, None


def extract_station_parts(raw_text: str) -> tuple[Optional[str], Optional[str]]:
    parts = [line.strip() for line in raw_text.splitlines() if line.strip()]
    if len(parts) >= 2:
        return parts[0], parts[1]
    return None, None


def parse_seat_cells(row) -> List[TrainSeatPrice]:
    seats: List[TrainSeatPrice] = []
    seat_types = [
        "商务座",
        "特等座",
        "一等座",
        "二等座",
        "高级软卧",
        "软卧",
        "硬卧",
        "软座",
        "硬座",
        "无座",
    ]

    for seat_type in seat_types:
        try:
            cell = row.find_element(By.XPATH, f".//*[contains(@title, '{seat_type}') or contains(text(), '{seat_type}')]")
        except NoSuchElementException:
            continue

        text = cell.text.strip()
        match = re.search(r"(\d+(?:\.\d+)?)", text)
        if not match:
            continue

        seats.append(
            TrainSeatPrice(
                seat_type=seat_type,
                price=float(match.group(1)),
                available_seats=None,
            )
        )

    return seats


async def fetch_trains_12306(
    from_city: str,
    to_city: str,
    travel_date: str,
) -> List[ScrapedTrain]:
    """从 12306 页面抓取火车票信息。

    当前实现是最小可用版本：
    - 优先复用数据库中的站码映射
    - 通过 Selenium 加载查询页
    - 尽量从表格中提取车次、站点、时间、时长、席别价格
    - 若页面风控、结构变化或浏览器不可用，则返回空结果，由上层决定是否 fallback
    """
    from_code = get_station_code(from_city)
    to_code = get_station_code(to_city)

    if not from_code or not to_code:
        logger.error("无法找到车站代码: %s -> %s", from_city, to_city)
        return []

    date_obj = datetime.strptime(travel_date, "%Y-%m-%d")
    formatted_date = date_obj.strftime("%Y-%m-%d")
    url = (
        "https://kyfw.12306.cn/otn/leftTicket/init"
        f"?linktypeid=dc&fs={from_code}&ts={to_code}&date={formatted_date}&flag=N,N,Y"
    )

    options = webdriver.ChromeOptions()
    options.add_argument("--headless=new")
    options.add_argument("--no-sandbox")
    options.add_argument("--disable-dev-shm-usage")
    options.add_argument("--window-size=1440,900")
    options.add_argument(
        "--user-agent=Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 "
        "(KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36"
    )

    driver = None
    trains: List[ScrapedTrain] = []

    try:
        logger.info("Fetching trains from 12306 with Selenium: %s", url)
        driver = webdriver.Chrome(options=options)
        driver.get(url)

        wait = WebDriverWait(driver, 15)
        wait.until(EC.presence_of_element_located((By.ID, "queryLeftTable")))

        train_rows = driver.find_elements(By.CSS_SELECTOR, "#queryLeftTable tbody tr")
        for row in train_rows:
            row_text = row.text.strip()
            if not row_text:
                continue

            try:
                train_id_match = re.search(r"\b([GDCKTZ]\d+)\b", row_text)
                if not train_id_match:
                    continue
                train_id = train_id_match.group(1)

                depart_time, arrive_time = extract_time_parts(row_text)
                if not depart_time or not arrive_time:
                    continue

                from_station, to_station = extract_station_parts(row_text)
                duration_match = re.search(r"(\d{2}:\d{2}|\d+小时\d+分|\d+小时|\d+分)", row_text.split(arrive_time, 1)[-1])
                duration_minutes = parse_duration(duration_match.group(1)) if duration_match else 0
                seats = parse_seat_cells(row)

                trains.append(
                    ScrapedTrain(
                        train_id=train_id,
                        train_type=parse_train_type(train_id),
                        from_station=from_station or from_city,
                        to_station=to_station or to_city,
                        from_city=from_city,
                        to_city=to_city,
                        depart_time=depart_time,
                        arrive_time=arrive_time,
                        duration_minutes=duration_minutes,
                        distance_km=None,
                        seats=seats,
                    )
                )
            except (NoSuchElementException, ValueError) as e:
                logger.warning("Failed to parse train row: %s", e)
                continue

        logger.info("Fetched %d trains from 12306", len(trains))
    except TimeoutException:
        logger.error("Timeout waiting for 12306 page to load")
    except WebDriverException as e:
        logger.error("Selenium browser unavailable: %s", e)
    except Exception as e:
        logger.exception("Error fetching trains from 12306: %s", e)
    finally:
        if driver:
            driver.quit()

    return trains


async def fetch_trains(
    from_city: str,
    to_city: str,
    travel_date: str,
) -> List[ScrapedTrain]:
    mode = current_train_fetch_mode()

    if mode == "mock":
        return await fetch_trains_mock(from_city, to_city, travel_date)

    trains = await fetch_trains_12306(from_city, to_city, travel_date)
    if trains:
        return trains

    if mode == "real":
        return []

    logger.warning(
        "Falling back to mock train data after real fetch returned empty results: %s -> %s on %s",
        from_city,
        to_city,
        travel_date,
    )
    return await fetch_trains_mock(from_city, to_city, travel_date)


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
