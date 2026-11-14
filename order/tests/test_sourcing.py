"""The distributor-lookup budget: one quote or install cannot spend it all, and a lookup that could not
be made is reported as unavailable, never as a part that can't be bought."""
import io
import json

import pytest

from fabsrv import config, pricing, sourcing
from fabsrv.sourcing import UNAVAILABLE


@pytest.fixture
def mouser(monkeypatch):
    calls = []

    class Resp(io.BytesIO):
        def __enter__(self):
            return self

        def __exit__(self, *a):
            return False

    def fake(req, timeout=0):
        calls.append(json.loads(req.data)["SearchByPartRequest"]["mouserPartNumber"].split("|"))
        return Resp(json.dumps({"SearchResults": {"Parts": []}}).encode())

    monkeypatch.setattr(config, "MOUSER_KEY", "k")
    monkeypatch.setattr(sourcing.urllib.request, "urlopen", fake)
    monkeypatch.setattr(sourcing, "_calls", [])
    monkeypatch.setattr(sourcing, "_cache", {})
    return calls


def test_an_answer_of_no_match_is_not_unavailable(mouser):
    assert sourcing.mouser(["A1"], "inst") == {"A1": None}


def test_budget_spent_is_unavailable_not_unbuyable(mouser, monkeypatch):
    monkeypatch.setattr(sourcing, "_calls", [(sourcing.time.time(), "other")] * sourcing.DAY_BUDGET)
    assert sourcing.mouser(["A1", "A2"], "inst") == {"A1": UNAVAILABLE, "A2": UNAVAILABLE}
    assert mouser == []


def test_no_key_or_a_failed_call_is_unavailable(mouser, monkeypatch):
    monkeypatch.setattr(config, "MOUSER_KEY", "")
    assert sourcing.mouser(["A1"]) == {"A1": UNAVAILABLE}


def test_one_quote_cannot_spend_more_than_its_share(mouser):
    # 100 unique part numbers is 10 calls: far under the share, so squeeze the share to see the cap act
    sourcing.SHARE, old = 3, sourcing.SHARE
    try:
        out = sourcing.mouser([f"P{i}" for i in range(60)], "inst")
    finally:
        sourcing.SHARE = old
    assert len(mouser) == 3
    assert sum(1 for v in out.values() if v is UNAVAILABLE) == 30
    assert sum(1 for v in out.values() if v is None) == 30


def test_one_install_cannot_spend_more_than_its_share(mouser, monkeypatch):
    monkeypatch.setattr(sourcing, "SHARE", 2)
    sourcing.mouser([f"A{i}" for i in range(30)], "inst")
    assert len(mouser) == 2
    out = sourcing.mouser(["B1"], "inst")
    assert out == {"B1": UNAVAILABLE} and len(mouser) == 2
    assert sourcing.mouser(["C1"], "someone-else") == {"C1": None}


def test_a_quote_looks_up_at_most_max_mpns_unique_parts(mouser):
    mpns = [f"P{i % 150}" for i in range(2000)]  # 2,000 lines, 150 unique
    out = sourcing.mouser(mpns, "inst")
    looked = [m for c in mouser for m in c]
    assert len(looked) == sourcing.MAX_MPNS == len(set(looked))
    assert sum(1 for v in out.values() if v is UNAVAILABLE) == 50


class Pack:
    def library(self, keys):
        return {k: None for k in keys}

    def sourcing_fees(self, lines):
        return {"total": 0.0}


def test_unavailable_parts_are_named_apart_from_problems(mouser, monkeypatch):
    monkeypatch.setattr(sourcing, "_calls", [(sourcing.time.time(), "x")] * sourcing.DAY_BUDGET)
    lines = [{"refs": ["R1"], "qty": 1, "mpn": "A1", "maker": "", "code": ""}]
    comp = sourcing.components(Pack(), lines, 5, "inst")
    assert comp["problems"] == [] and len(comp["unavailable"]) == 1
    assert "unavailable right now" in comp["unavailable"][0] and "can't be bought" not in comp["unavailable"][0]
    assert comp["lines"][0]["status"] == "unavailable"


def test_a_part_the_distributor_does_not_have_is_still_a_problem(mouser):
    lines = [{"refs": ["R1"], "qty": 1, "mpn": "A1", "maker": "", "code": ""}]
    comp = sourcing.components(Pack(), lines, 5, "inst")
    assert len(comp["problems"]) == 1 and comp["unavailable"] == []


def test_the_quote_text_says_retry_not_change_the_board():
    q = {"total": 1.0, "country_name": "United States", "region": "", "lead_days": 0, "shipping": {}, "lines": [],
         "parts": [], "problems": [], "warnings": [], "unavailable": ["R1 (A1): sourcing is unavailable right now"]}
    text = pricing.markdown(q)
    assert "Can't be priced right now" in text and "To fix before ordering" not in text
