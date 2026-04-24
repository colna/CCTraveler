"""12306 train ticket fetcher with undetected-chromedriver."""
from __future__ import annotations

import asyncio
import logging
import os
import random
import re
import time
from datetime import datetime
from typing import List, Optional

from selenium.webdriver.common.by import By
from selenium.webdriver.support.ui import WebDriverWait
from selenium.webdriver.support import expected_conditions as EC
from selenium.common.exceptions import (
    TimeoutException,
    NoSuchElementException,
    WebDriverException,
)

from ..utils.geo_lookup import get_station_code
from .types import ScrapedTrain, TrainSeatPrice

logger = logging.getLogger(__name__)

TRAIN_FETCH_MODE_ENV = "CCTRAVELER_TRAIN_FETCH_MODE"
MAX_RETRIES = 2


def parse_train_type(train_id: str) -> str:
    prefix = train_id[0] if train_id else ""
    return prefix if prefix in ("G", "D", "C", "K", "T", "Z") else "其他"


def parse_duration(duration_str: str) -> int:
    """Parse duration string to minutes.

    Examples: "08:30" -> 510, "1小时30分" -> 90
    """
    if ":" in duration_str:
        parts = duration_str.split(":")
        try:
            return int(parts[0]) * 60 + int(parts[1])
        except ValueError:
            return 0

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
        "商务座", "特等座", "一等座", "二等座",
        "高级软卧", "软卧", "硬卧", "软座", "硬座", "无座",
    ]

    for seat_type in seat_types:
        try:
            cell = row.find_element(
                By.XPATH,
                f".//*[contains(@title, '{seat_type}') or contains(text(), '{seat_type}')]",
            )
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


def _create_driver():
    """Create a browser driver, preferring undetected-chromedriver."""
    try:
        import undetected_chromedriver as uc

        options = uc.ChromeOptions()
        options.add_argument("--headless=new")
        options.add_argument("--no-sandbox")
        options.add_argument("--disable-dev-shm-usage")
        options.add_argument("--window-size=1440,900")
        driver = uc.Chrome(options=options)
        logger.info("Using undetected-chromedriver")
        return driver
    except Exception as e:
        logger.warning("undetected-chromedriver unavailable (%s), falling back to selenium", e)
        from selenium import webdriver

        options = webdriver.ChromeOptions()
        options.add_argument("--headless=new")
        options.add_argument("--no-sandbox")
        options.add_argument("--disable-dev-shm-usage")
        options.add_argument("--window-size=1440,900")
        options.add_argument(
            "--user-agent=Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
            "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36"
        )
        driver = webdriver.Chrome(options=options)
        return driver


async def fetch_trains_12306(
    from_city: str,
    to_city: str,
    travel_date: str,
) -> List[ScrapedTrain]:
    """Fetch train tickets from 12306 with retry and anti-detection.

    Uses undetected-chromedriver when available to bypass anti-bot measures.
    Retries up to MAX_RETRIES times on failure before returning empty.
    """
    from_code = get_station_code(from_city)
    to_code = get_station_code(to_city)

    if not from_code or not to_code:
        logger.error("Cannot resolve station codes: %s -> %s", from_city, to_city)
        return []

    date_obj = datetime.strptime(travel_date, "%Y-%m-%d")
    formatted_date = date_obj.strftime("%Y-%m-%d")
    url = (
        "https://kyfw.12306.cn/otn/leftTicket/init"
        f"?linktypeid=dc&fs={from_code}&ts={to_code}&date={formatted_date}&flag=N,N,Y"
    )

    last_error = None
    for attempt in range(1, MAX_RETRIES + 1):
        driver = None
        trains: List[ScrapedTrain] = []

        try:
            logger.info(
                "Fetching trains from 12306 (attempt %d/%d): %s",
                attempt, MAX_RETRIES, url,
            )
            driver = _create_driver()

            # Random delay to mimic human behavior
            await asyncio.sleep(random.uniform(0.5, 2.0))

            driver.get(url)

            wait = WebDriverWait(driver, 20)
            wait.until(EC.presence_of_element_located((By.ID, "queryLeftTable")))

            # Wait for dynamic content
            await asyncio.sleep(random.uniform(1.0, 2.0))

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
                    duration_text = row_text.split(arrive_time, 1)[-1] if arrive_time else ""
                    duration_match = re.search(
                        r"(\d{2}:\d{2}|\d+小时\d+分|\d+小时|\d+分)", duration_text
                    )
                    duration_minutes = (
                        parse_duration(duration_match.group(1)) if duration_match else 0
                    )
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

            logger.info("Fetched %d trains from 12306 (attempt %d)", len(trains), attempt)

            if trains:
                return trains

            # Detect anti-bot page
            page_source = driver.page_source or ""
            if "网络繁忙" in page_source or "验证" in page_source:
                logger.warning("12306 anti-bot page detected on attempt %d", attempt)
                last_error = "anti-bot"
            else:
                logger.warning("No trains found in page on attempt %d", attempt)
                last_error = "empty"

        except TimeoutException:
            logger.warning("Timeout waiting for 12306 page (attempt %d)", attempt)
            last_error = "timeout"
        except WebDriverException as e:
            logger.error("Browser error (attempt %d): %s", attempt, e)
            last_error = str(e)
            break  # Don't retry on browser unavailable
        except Exception as e:
            logger.exception("Unexpected error fetching trains (attempt %d): %s", attempt, e)
            last_error = str(e)
        finally:
            if driver:
                try:
                    driver.quit()
                except Exception:
                    pass

        if attempt < MAX_RETRIES:
            delay = random.uniform(2.0, 5.0)
            logger.info("Retrying in %.1f seconds...", delay)
            await asyncio.sleep(delay)

    logger.error("All %d attempts failed for 12306 (%s)", MAX_RETRIES, last_error)
    return []


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
        "Falling back to mock train data: %s -> %s on %s",
        from_city, to_city, travel_date,
    )
    return await fetch_trains_mock(from_city, to_city, travel_date)


async def fetch_trains_mock(
    from_city: str,
    to_city: str,
    travel_date: str,
) -> List[ScrapedTrain]:
    """Mock data for development and testing."""
    logger.info("Using mock data for %s -> %s on %s", from_city, to_city, travel_date)
    await asyncio.sleep(0.5)

    return [
        ScrapedTrain(
            train_id="G1234",
            train_type="G",
            from_station=f"{from_city}站",
            to_station=f"{to_city}站",
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
            from_station=f"{from_city}站",
            to_station=f"{to_city}站",
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
