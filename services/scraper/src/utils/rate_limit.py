"""Request rate limiting."""
from __future__ import annotations

import asyncio
import random


async def random_delay(min_sec: float = 2.0, max_sec: float = 5.0) -> None:
    """Sleep for a random duration to avoid detection."""
    delay = random.uniform(min_sec, max_sec)
    await asyncio.sleep(delay)
