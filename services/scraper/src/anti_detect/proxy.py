"""Proxy pool: pick proxies from env var SCRAPER_PROXY_POOL (comma-separated)."""
from __future__ import annotations

import os
import random
from typing import List, Optional


def _load_pool() -> List[str]:
    raw = os.environ.get("SCRAPER_PROXY_POOL", "").strip()
    if not raw:
        return []
    return [p.strip() for p in raw.split(",") if p.strip()]


_POOL: List[str] = _load_pool()


def pick_proxy() -> Optional[str]:
    """Return a random proxy URL, or None if no pool configured.

    Format: http://user:pass@host:port  or  http://host:port
    """
    if not _POOL:
        return None
    return random.choice(_POOL)


def pool_size() -> int:
    return len(_POOL)
