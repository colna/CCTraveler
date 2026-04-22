"""Basic tests for Ctrip parser."""
from src.ctrip.parser import parse_hotel_list


def test_parse_empty_html():
    result = parse_hotel_list("<html><body></body></html>", city="test")
    assert result == []
