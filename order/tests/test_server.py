"""The order server against a stand-in fab pack (tests/fakevendor): estimates, uploads and
their checks, tokens, limits, and the measuring of a small board."""
import hashlib
import io
import json
import time
import zipfile

from fastapi.testclient import TestClient

from fabsrv import analysis, app as appmod, limits, package

INSTALL = {"X-Backplane-Install": "test-install-0123456789"}


def client():
    return TestClient(appmod.app)


# a 20 x 15 mm two-layer board: outline, both coppers, a top paste layer with 3 pads, a drill file
def gerber(function, body):
    return f"%FSLAX46Y46*%\n%MOMM*%\n%TF.FileFunction,{function}*%\n%ADD10C,0.100000*%\nD10*\n{body}M02*\n"


OUTLINE = "X0Y0D02*\nX20000000Y0D01*\nX20000000Y15000000D01*\nX0Y15000000D01*\nX0Y0D01*\n"
PADS = "X1000000Y1000000D03*\nX2000000Y1000000D03*\nX3000000Y1000000D03*\n"
DRILL = "M48\n;TYPE=PLATED\nMETRIC\nT1C0.300\n%\nT1\nX5.0Y5.0\nX6.0Y5.0\nM30\n"


def board_zip(layers=2):
    b = io.BytesIO()
    with zipfile.ZipFile(b, "w") as z:
        z.writestr("b-Edge_Cuts.gm1", gerber("Profile,NP", OUTLINE))
        z.writestr("b-F_Cu.gtl", gerber("Copper,L1,Top", ""))
        for i in range(2, layers):
            z.writestr(f"b-In{i - 1}_Cu.g{i}", gerber(f"Copper,L{i},Inr", ""))
        z.writestr("b-B_Cu.gbl", gerber(f"Copper,L{layers},Bot", ""))
        z.writestr("b-F_Paste.gtp", gerber("Paste,Top", PADS))
        z.writestr("b-PTH.drl", DRILL)
    return b.getvalue()


BOM = b"Designator,Quantity,MPN\nR1-R3,3,RC0402-10K\n"
POS = b"Ref,Val,Package,PosX,PosY,Rot,Side\nR1,10k,0402,1,1,0,top\nR2,10k,0402,2,1,0,top\nR3,10k,0402,3,1,0,top\n"


def manifest(sent, **part):
    p = {"name": "Main board", "kind": "pcb", "qty": 5, "options": {"layers": 2},
         "files": {"gerbers": "main/gerbers.zip"}, "assemble": 0}
    p.update(part)
    return {"spec": 1, "title": "Test", "ship_to": "US", "parts": [p],
            "files": {k: hashlib.sha256(v).hexdigest() for k, v in sent.items()}}


def upload(c, man, files, headers=INSTALL):
    return c.post("/v1/quotes", headers=headers, data={"manifest": json.dumps(man)},
                  files=[("file", (k, v)) for k, v in files.items()])


def wait_ready(c, qid, token):
    for _ in range(100):
        r = c.get(f"/v1/quotes/{qid}", headers={"Authorization": f"Bearer {token}"}).json()
        if r["status"] not in ("received", "analysing"):
            return r
        time.sleep(0.1)
    raise AssertionError("quote never finished")


def test_health_and_catalog():
    with client() as c:
        assert c.get("/v1/health").json()["pricing"] is True
        kinds = c.get("/v1/catalog").json()["kinds"]
        assert set(kinds) == {"pcb", "flex", "hdi", "aluminum", "copper_core", "rogers", "ptfe", "3dp", "cnc", "sheetmetal"}
        assert all(kinds[k]["min_qty"] == 5 for k in ("pcb", "flex", "hdi", "aluminum", "copper_core", "rogers", "ptfe"))


def test_estimate_prices_parts_and_names_problems():
    with client() as c:
        r = c.post("/v1/estimate", headers=INSTALL, json={"parts": [
            {"name": "b", "kind": "pcb", "options": {"layers": 2}, "features": {"width_mm": 20, "height_mm": 15}},
            {"name": "x", "kind": "pcb", "options": {"colour": "red"}, "features": {"width_mm": 20, "height_mm": 15}}]}).json()
        assert r["parts"][0]["qty"] == 5 and r["label"].startswith("from $")
        assert any("unknown option" in p for p in r["parts"][1]["problems"])
        assert c.post("/v1/estimate", json={"parts": []}).status_code == 400  # no install id


BOARD = {"width_mm": 20, "height_mm": 15}


def test_every_board_kind_is_estimated():
    with client() as c:
        kinds = ["pcb", "flex", "hdi", "aluminum", "copper_core", "rogers", "ptfe"]
        r = c.post("/v1/estimate", headers=INSTALL, json={"parts": [
            {"name": k, "kind": k, "options": {}, "features": BOARD} for k in kinds]}).json()
        assert [p.get("qty") for p in r["parts"]] == [5] * len(kinds), r
        assert all("from" in p and not p.get("problems") for p in r["parts"]), r


def test_board_kind_rules():
    from fabsrv import catalog
    # a flex board's thickness follows its layers; left out, it is the layer count's own
    opts, _, probs = catalog.clean("flex", {"layers": 1}, BOARD)
    assert not probs and opts["thickness_mm"] == 0.07
    assert catalog.clean("flex", {"layers": 4, "thickness_mm": 0.11}, BOARD)[2] == [
        "a 4-layer flex board is 0.2, 0.25, 0.3, 0.35, 0.45 mm thick, not 0.11"]
    # many-layer HDI: what the person didn't choose follows the board; what they did is a problem
    opts, _, probs = catalog.clean("hdi", {"layers": 8}, BOARD)
    assert not probs and (opts["finish"], opts["via_fill"], opts["laminate"]) == ("enig", "epoxy_filled", "s1000_2m")
    assert catalog.clean("hdi", {"layers": 8, "finish": "hasl"}, BOARD)[2] == [
        "an 8-layer HDI board's finish is enig or osp, not hasl"]
    assert catalog.clean("hdi", {"layers": 4, "steps": 2}, BOARD)[2] == ["a 4-layer HDI board has 1 build-up step, not 2"]
    # 0.8 mm aluminium only comes lead-free
    assert catalog.clean("aluminum", {"thickness_mm": 0.8}, BOARD)[0]["finish"] == "hasl_lead_free"
    # rigid FR-4 is as it was
    assert catalog.clean("pcb", {}, BOARD)[0] == {"layers": 2, "thickness_mm": 1.6, "color": "green", "finish": "hasl",
                                                  "copper_oz": 1, "castellated": False, "gold_fingers": False}


def quote_of(c, man, files):
    r = upload(c, man, files)
    assert r.status_code == 202, r.text
    up = r.json()
    return wait_ready(c, up["id"], up["token"])


def test_flex_board_quoted_from_its_files():
    files = {"flex/gerbers.zip": board_zip()}
    man = manifest(files, name="Tail", kind="flex", options={"layers": 2, "stiffener": "fr4_0.2"},
                   files={"gerbers": "flex/gerbers.zip"})
    with client() as c:
        q = quote_of(c, man, files)
        assert q["status"] == "ready", q
        quote = q["quote"]
        assert quote["parts"][0]["summary"] == "2-layer flex board, 20×15 mm, 0.11 mm thick, yellow coverlay, 0.2 mm FR-4 stiffener"
        assert quote["lines"][0]["detail"] == "Boards" and not quote["problems"]


def test_board_kinds_check_their_layers():
    files = {"al/gerbers.zip": board_zip()}  # two copper layers
    with client() as c:
        q = quote_of(c, manifest(files, name="Lamp", kind="aluminum", options={}, files={"gerbers": "al/gerbers.zip"}), files)
        assert q["quote"]["problems"] == ["Lamp: the gerbers have 2 copper layers, and an aluminium board has 1"], q
        # HDI said 4 layers and the gerbers have 2: 2 can't be HDI
        q = quote_of(c, manifest(files, name="Hdi", kind="hdi", options={"layers": 4}, files={"gerbers": "al/gerbers.zip"}), files)
        assert q["quote"]["problems"] == ["Hdi: the gerbers have 2 copper layers, and an HDI board has 4 to 16"], q
        # HDI said 4 layers and the gerbers have 6: priced as 6, with what 6 layers come with
        six = {"hdi/gerbers.zip": board_zip(6)}
        q = quote_of(c, manifest(six, name="Hdi", kind="hdi", options={"layers": 4}, files={"gerbers": "hdi/gerbers.zip"}), six)
        quote = q["quote"]
        assert not quote["problems"] and quote["parts"][0]["summary"].startswith("6-layer HDI board (1-step)"), q
        assert "Hdi: at 6 layers it comes with finish ENIG, via fill epoxy-filled and capped" in quote["warnings"], quote["warnings"]
        # a flex thickness left out follows the layers the gerbers have; one chosen that can't be is a problem
        two = {"flex/gerbers.zip": board_zip(2)}
        q = quote_of(c, manifest(two, name="Tail", kind="flex", options={"layers": 1}, files={"gerbers": "flex/gerbers.zip"}), two)
        assert not q["quote"]["problems"] and "Tail: at 2 layers it comes with 0.11 mm thickness" in q["quote"]["warnings"], q
        q = quote_of(c, manifest(two, name="Tail", kind="flex", options={"layers": 1, "thickness_mm": 0.07},
                                 files={"gerbers": "flex/gerbers.zip"}), two)
        assert q["quote"]["problems"] == ["Tail: a 2-layer flex board is 0.11, 0.12, 0.19, 0.2, 0.24 mm thick, not 0.07"], q


def test_only_rigid_boards_are_assembled():
    files = {"flex/gerbers.zip": board_zip(), "flex/bom.csv": BOM, "flex/pos.csv": POS}
    man = manifest(files, kind="flex", options={"layers": 2}, assemble=2,
                   files={"gerbers": "flex/gerbers.zip", "bom": "flex/bom.csv", "positions": "flex/pos.csv"})
    with client() as c:
        r = upload(c, man, files)
        assert r.status_code == 400 and "only rigid FR-4 boards (pcb) are assembled" in r.text, r.text


def test_quote_from_files_with_assembly():
    files = {"main/gerbers.zip": board_zip(), "main/bom.csv": BOM, "main/pos.csv": POS}
    man = manifest(files, files={"gerbers": "main/gerbers.zip", "bom": "main/bom.csv", "positions": "main/pos.csv"}, assemble=2)
    with client() as c:
        r = upload(c, man, files)
        assert r.status_code == 202, r.text
        up = r.json()
        q = wait_ready(c, up["id"], up["token"])
        assert q["status"] == "ready", q
        quote = q["quote"]
        part = quote["parts"][0]
        assert "20×15 mm" in part["summary"] and part["assembled"] == 2
        # the fee is inside Manufacturing, never a line of its own
        assert [ln["label"] for ln in quote["lines"]] == ["Manufacturing", "Shipping", "Duties & taxes", "Sales tax (estimated)"]
        cost = sum(pl["amount"] for pl in quote.get("part_lines", []))
        assert quote["lines"][0]["amount"] > cost, "Manufacturing carries the fee on top of the fab's cost"
        assert abs(sum(ln["amount"] for ln in quote["lines"]) - quote["total"]) < 0.01
        assert "part_lines" not in quote  # what each part costs stays with us
        assert quote["pay_label"].startswith("Buy now · $") and "Your quote" in quote["markdown"]
        # the token is the only way in
        assert c.get(f"/v1/quotes/{up['id']}").status_code == 404
        assert c.get(f"/v1/quotes/{up['id']}", headers={"Authorization": "Bearer wrong"}).status_code == 404
        assert c.get(f"/q/{up['id']}?t=nope").status_code == 404
        # Buy now goes straight to Stripe; payments are not set up in tests: a clear refusal, nothing else
        assert c.get(f"/q/{up['id']}?t={up['token']}", follow_redirects=False).status_code == 503
        # paying paused: Buy now says so instead of opening a payment page
        appmod.config.PAUSED = True
        try:
            r = c.get(f"/q/{up['id']}?t={up['token']}", follow_redirects=False)
            assert r.status_code == 200 and "paused" in r.text and "export the manufacturing files" in r.text
        finally:
            appmod.config.PAUSED = False


def test_a_part_only_standard_assembly_places_prices_it():
    # a BGA/LGA part or a module goes only on the fab's standard (dearer)
    # assembly service; priced as the cheaper one, the quote came in a fifth
    # under the fab's invoice
    bom = BOM + b"U1,1,LGA-IMU\n"
    pos = POS + b"U1,imu,LGA-14,4,1,0,top\n"
    quotes = []
    for b, p in ((BOM, POS), (bom, pos)):
        files = {"main/gerbers.zip": board_zip(), "main/bom.csv": b, "main/pos.csv": p}
        man = manifest(files, files={"gerbers": "main/gerbers.zip", "bom": "main/bom.csv", "positions": "main/pos.csv"}, assemble=2)
        with client() as c:
            up = upload(c, man, files).json()
            q = wait_ready(c, up["id"], up["token"])
            assert q["status"] == "ready", q
            quotes.append(q["quote"])
    plain, standard = quotes
    assert not any("standard assembly" in w for w in plain["warnings"]), plain["warnings"]
    assert any("standard assembly" in w and "LGA-IMU" in w for w in standard["warnings"]), standard["warnings"]
    # the IMU itself is 1.00 a board; the rest is the dearer service
    assert standard["total"] - plain["total"] > 25, (plain["total"], standard["total"])


def test_uploads_are_checked():
    good = {"main/gerbers.zip": board_zip()}
    with client() as c:
        bad = manifest(good)
        bad["files"] = {"../etc/passwd.zip": bad["files"]["main/gerbers.zip"]}
        assert upload(c, bad, good).status_code == 400
        wrong = manifest(good)
        wrong["files"]["main/gerbers.zip"] = "0" * 64
        assert "sha256" in upload(c, wrong, good).json()["error"]
        extra = dict(good, **{"main/extra.csv": b"x"})
        assert "not in the manifest" in upload(c, manifest(good), extra).json()["error"]
        exe = {"main/run.sh": b"#!/bin/sh"}
        assert "not accepted" in upload(c, manifest(exe, files={"gerbers": "main/run.sh"}), exe).json()["error"]
        assert "assembly needs" in upload(c, manifest(good, assemble=2), good).json()["error"]
        assert "assembly starts" in upload(c, manifest(good, assemble=1), good).json()["error"]


def test_zip_links_and_bombs_refused(tmp_path):
    z = io.BytesIO()
    with zipfile.ZipFile(z, "w") as f:
        info = zipfile.ZipInfo("link.gbr")
        info.external_attr = 0o120777 << 16
        f.writestr(info, "/etc/passwd")
    p = tmp_path / "a.zip"
    p.write_bytes(z.getvalue())
    try:
        package._unzip(p, tmp_path, [10 ** 9])
        raise AssertionError("a link was unpacked")
    except package.Bad as e:
        assert "links" in str(e)
    z = io.BytesIO()
    with zipfile.ZipFile(z, "w", zipfile.ZIP_DEFLATED) as f:
        f.writestr("big.gbr", b"0" * (60 * 1024 * 1024))
    p.write_bytes(z.getvalue())
    try:
        package._unzip(p, tmp_path, [10 ** 9])
        raise AssertionError("a bomb was unpacked")
    except package.Bad as e:
        assert "too large" in str(e)


def test_limits_count_an_ipv6_block_as_one():
    assert limits.client_key("2001:db8:1:2::5") == limits.client_key("2001:db8:1:2:ffff::9")
    assert limits.client_key("::ffff:10.0.0.1") == "ip:10.0.0.1"
    for _ in range(limits.RULES["pay"][0][1]):
        assert limits.check("pay", "198.51.100.7") == 0
    assert limits.check("pay", "198.51.100.7") > 0


def test_measuring_a_board(tmp_path):
    (tmp_path / "g").mkdir()
    with zipfile.ZipFile(io.BytesIO(board_zip())) as z:
        z.extractall(tmp_path / "g")
    b, problems = analysis.board(tmp_path, "g")
    assert not problems
    assert (b["layers"], b["width_mm"], b["height_mm"], b["paste_top"], b["min_drill_mm"], b["holes"]) == (2, 20.0, 15.0, 3, 0.3, 2)
    assert analysis.refs_of("C1,C3 C6-C8") == ["C1", "C3", "C6", "C7", "C8"]


def test_pilot_says_no_card_is_charged(monkeypatch):
    from fabsrv import config, pages, pricing
    monkeypatch.setattr(config, "PILOT", True)
    done = pages.done("paid")
    assert "not charged" in done and "fabricator" in done
    q = {"total": 10.0, "shipping": {"method": "", "days": ""}, "country_name": "Greece", "lead_days": 0,
         "lines": [{"label": "Manufacturing", "amount": 10.0}], "parts": [], "problems": [], "warnings": []}
    md = pricing.markdown(q)
    assert "**Pilot:**" in md and "will not be charged" in md and "mfg/" in md
    assert "4242 4242 4242 4242" in config.PILOT_NOTE and len(config.PILOT_NOTE) <= 1200
    monkeypatch.setattr(config, "PILOT", False)
    assert "not charged" not in pages.done("paid") and "**Pilot:**" not in pricing.markdown(q)


def test_ship_to_must_be_a_country():
    sent = {"main/gerbers.zip": b"x"}
    for code, says in (("EU", "European Union"), ("UK", "GB"), ("XX", "not a country")):
        m = manifest(sent)
        m["ship_to"] = code
        try:
            package.check_manifest(m)
            raise AssertionError(f"{code} passed as a country")
        except package.Bad as e:
            assert says in str(e) and "ask which country" in str(e), str(e)
    m = manifest(sent)
    m["ship_to"] = "GR"
    assert package.check_manifest(m)["ship_to"] == "GR"


# ---------------------------------------------------------------- taxes and fees the fab charges on top

def test_us_sales_tax_follows_the_state():
    files = {"main/gerbers.zip": board_zip()}
    with client() as c:
        quotes = {}
        for region in ("GA", "OR", None):
            man = manifest(files)
            if region:
                man["ship_region"] = region
            quotes[region] = quote_of(c, man, files)["quote"]
    ga, orgn, none = quotes["GA"], quotes["OR"], quotes[None]
    tax = {k: q["lines"][3] for k, q in quotes.items()}
    # on the goods (the stand-in board, 3.50) and the shipping (20.00), at Georgia's average combined rate
    assert tax["GA"]["label"] == "Sales tax (estimated)" and "Georgia, 7.56%" in tax["GA"]["detail"]
    assert ga["sales_tax"]["rate"] == 7.56 and ga["region"] == "GA" and ga["sales_tax"]["base"] == 23.50
    assert tax["GA"]["amount"] == round(ga["sales_tax"]["base"] * 0.0756, 2)
    assert "Ships to the United States (Georgia)" in ga["markdown"]
    # Oregon has none; without a state, a high rate, and the quote says so
    assert tax["OR"]["amount"] == 0 and "Oregon has no sales tax" in tax["OR"]["detail"]
    assert none["sales_tax"]["rate"] == 10.25 and none["region"] == "" and "without the state" in tax[None]["detail"]
    assert any("doesn't say which US state" in w for w in none["warnings"]), none["warnings"]
    assert none["total"] > ga["total"] > orgn["total"]
    for q in quotes.values():
        assert abs(sum(ln["amount"] for ln in q["lines"]) - q["total"]) < 0.01


def test_ship_region_is_a_us_state_and_only_read_for_the_us():
    sent = {"main/gerbers.zip": b"x"}
    for bad in ("XX", "Georgia", 7):
        m = manifest(sent)
        m["ship_region"] = bad
        try:
            package.check_manifest(m)
            raise AssertionError(f"{bad!r} passed as a state")
        except package.Bad as e:
            assert "two-letter code" in str(e), str(e)
    m = manifest(sent)
    m["ship_region"] = " wa"
    assert package.check_manifest(m)["ship_region"] == "WA"
    m = manifest(sent)
    m["ship_to"], m["ship_region"] = "GR", "Attica"
    assert package.check_manifest(m)["ship_region"] == ""


def test_duty_paid_parcels_carry_the_couriers_fee():
    from fabsrv import duties
    assert duties.disbursement("US", 1, True) == 4.44
    assert duties.disbursement("GR", 1, True) == round(7.40 * duties.EUR_FEES, 2) >= 8.5
    assert duties.disbursement("US", 2, True) == 8.88
    assert duties.disbursement("US", 1, False) == 0 and duties.disbursement("US", 0, True) == 0
    files = {"main/gerbers.zip": board_zip()}
    with client() as c:
        us = quote_of(c, manifest(files), files)["quote"]
        man = manifest(files)
        man["ship_to"] = "GR"
        gr = quote_of(c, man, files)["quote"]
    # the stand-in board costs 2 + 20*15/1000*5 = 3.50; its duty-paid shipping 20.00
    assert us["lines"][2]["amount"] == round(3.50 * 0.35 + 4.44, 2), us["lines"]
    assert "courier's fee" in us["lines"][2]["detail"]
    assert gr["lines"][2]["amount"] == round((3.50 + 20) * 0.24 + duties.EUR_LINE + 8.88, 2), gr["lines"]
    # no sales tax outside the US
    assert [ln["label"] for ln in gr["lines"]] == ["Manufacturing", "Shipping", "Duties & taxes"] and gr["sales_tax"] is None


class _Pinned:
    """A pack pinned to what the real one priced two paid orders at (2026-11-08), by numbers only."""

    def __init__(self, board, assembly, ship):
        self.board, self.asm, self.ship = board, assembly, ship

    def min_qty(self, kind, options):
        return 5

    def estimate(self, kind, options, features, qty):
        return {"total": self.board, "lines": [], "lead_days": 3, "warnings": []}

    def assembly(self, spec):
        return {"total": self.asm, "lines": [], "lead_days": 4, "warnings": []}

    def shipping(self, country, parcels):
        return {"price": self.ship, "method": "Express (duties paid)", "days": "2-4 days", "ddp": True, "parcels": 1}


def _price_pinned(monkeypatch, pack, components, qty, assemble, region, size):
    from fabsrv import pricing, sourcing, vendor_api
    monkeypatch.setattr(vendor_api, "pack", lambda: pack)
    monkeypatch.setattr(sourcing, "components", lambda v, lines, boards: {
        "total": components, "unique": 14, "extended": 5, "lines": [], "problems": [], "warnings": [],
        "extra_days": 0, "standard_only": ["LGA"] if assemble else [], "xray_parts": 1 if assemble else 0})
    opts = {"layers": 2, "thickness_mm": 1.6, "color": "green", "finish": "hasl", "copper_oz": 1,
            "castellated": False, "gold_fingers": False}
    man = {"ship_to": "US", "ship_region": region, "title": "t", "parts": [
        {"name": "b", "kind": "pcb", "qty": qty, "options": opts, "features": {}, "files": {}, "assemble": assemble, "given": []}]}
    feats = {"parts": {"b": {"width_mm": size[0], "height_mm": size[1], "layers": 2, "min_drill_mm": 0.3, "paste_top": 88,
                             "tht_holes": 0, "bom": []}}, "problems": [], "warnings": []}
    return pricing.price(man, feats)


def test_the_two_orders_that_came_in_under_now_cover_the_fab(monkeypatch):
    # 2026-11-08: two paid US orders the fab charged more for than their quotes
    # (no sales tax, no duty-paid parcel fee, shipping under). With what the
    # pack prices them at now, the quote covers the fab's checkout by about the fee.
    # 5 assembled 2-layer boards to Georgia: quoted $258.79, the fab charged $265.07
    q = _price_pinned(monkeypatch, _Pinned(20.70, 72.65, 34.86), 71.66, 5, 5, "GA", (25.4, 20.32))
    assert 265.07 * 1.05 < q["total"] < 265.07 * 1.20, q["lines"]
    # 10 bare 2-layer boards to Washington: quoted $36.97, the fab charged $43.43
    q = _price_pinned(monkeypatch, _Pinned(5.00, 0, 29.26), 0, 10, 0, "WA", (99.06, 90.68))
    assert 43.43 * 1.05 < q["total"] < 43.43 * 1.15, q["lines"]
    assert [ln["label"] for ln in q["lines"]] == ["Manufacturing", "Shipping", "Duties & taxes", "Sales tax (estimated)"]
    assert abs(sum(ln["amount"] for ln in q["lines"]) - q["total"]) < 0.01


def test_the_order_list_flags_a_state_the_tax_was_not_quoted_for():
    from fabsrv import pages, pay
    q = {"sales_tax": {"region": "GA", "rate": 7.56, "base": 200.0, "amount": 15.12, "known": True}}
    assert pages.tax_flag(q, {"state": "GA"}) == ""
    flag = pages.tax_flag(q, {"state": "WA"})
    assert "quoted for GA (7.56%)" in flag and "ships to WA (9.57%)" in flag and "$4.02 more to ask for" in flag, flag
    assert "to refund" in pages.tax_flag(q, {"state": "OR"})
    none = {"sales_tax": {"region": "", "rate": 10.25, "base": 200.0, "amount": 20.5, "known": False}}
    assert "quoted for no state" in pages.tax_flag(none, {"state": "GA"})
    assert pages.tax_flag({}, {"state": "GA"}) == ""  # quoted before sales tax was
    row = {"result": json.dumps(q)}
    assert pay.shipped_state(row, {"country": "US", "state": "wa"}) == {"state": "WA", "quoted": "GA"}
    assert pay.shipped_state(row, {"country": "GR", "state": "Attica"}) == {}
