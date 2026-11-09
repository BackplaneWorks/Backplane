"""The sendcutsend skill's helper (deploy/skills/sendcutsend/scripts/scs.py), on recorded copies of SendCutSend's three
files (fixtures/, trimmed: make_fixtures.py --record) and a fake opener: no network. SCS_LIVE=1 adds one live fetch.
    python3 -I -m unittest discover -s test/skills/sendcutsend -v
The hub's tests run this too (agents::bundled)."""
import email.message, io, json, os, shutil, subprocess, sys, tempfile, time, unittest, urllib.error
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SKILL = ROOT / "deploy/skills/sendcutsend"
sys.path.insert(0, str(SKILL / "scripts"))
sys.path.insert(0, str(HERE))
import scs  # noqa: E402
import scs_sheet as S  # noqa: E402
import make_fixtures as MF  # noqa: E402

FIX = HERE / "fixtures"
NAMES = {n: f for n, (_, f) in scs.SOURCES.items()}


class Resp:
    def __init__(self, data, headers):
        self.data = data
        self.headers = email.message.Message()
        for k, v in headers.items():
            self.headers[k] = v

    def read(self):
        return self.data


class FakeNet:
    """Answers like the three hosts: ETag on the guide, Last-Modified on all; 304 when the validators match."""

    def __init__(self, bodies=None):
        self.bodies = bodies or {n: (FIX / f).read_bytes() for n, f in NAMES.items()}
        self.calls, self.down = [], False

    def __call__(self, req, timeout):
        self.calls.append(req)
        if self.down:
            raise urllib.error.URLError("Temporary failure in name resolution")
        name = next(n for n, (u, _) in scs.SOURCES.items() if u == req.full_url)
        body = self.bodies[name]
        etag = '"%s"' % scs.hashlib.md5(body).hexdigest() if name == "guide" else None
        lm = "Thu, 05 Nov 2026 22:30:56 GMT" if body == (FIX / NAMES[name]).read_bytes() else "Mon, 09 Nov 2026 10:00:00 GMT"
        h = {"Last-Modified": lm, **({"ETag": etag} if etag else {})}
        if (etag and req.get_header("If-none-match") == etag) or (not etag and req.get_header("If-modified-since") == lm):
            raise urllib.error.HTTPError(req.full_url, 304, "Not Modified", Resp(b"", h).headers, io.BytesIO(b""))
        return Resp(body, h)


class Clock:
    def __init__(self):
        self.t = float(int(time.time()))
        self.slept = []

    def __call__(self):
        return self.t

    def sleep(self, s):
        self.slept.append(s)
        self.t += s


def fetcher(tmp, net=None, clock=None, ttl=3600, offline=False):
    clock = clock or Clock()
    return scs.Fetcher(cache=tmp, ttl_s=ttl, offline=offline, opener=net or FakeNet(), now=clock, sleep=clock.sleep)


def seeded(tmp, specs_edit=None):
    """A cache holding the recorded fixtures, fresh, for the DFM tests (which never fetch)."""
    net = FakeNet()
    if specs_edit:
        o = json.loads(net.bodies["specs"])
        specs_edit(o)
        net.bodies["specs"] = json.dumps(o).encode()
    f = fetcher(tmp, net)
    for n in scs.SOURCES:
        f.get(n)
    f.opener = None  # any fetch from here on would fail the test
    return f


class FetchTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="scs-test-"))

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_first_fetch_downloads_and_keeps_the_validators(self):
        net = FakeNet()
        f = fetcher(self.tmp, net)
        g = f.get("guide")
        f.get("catalog")
        self.assertEqual(g["how"], "downloaded")
        meta = json.loads((self.tmp / (NAMES["guide"] + ".meta.json")).read_text())
        self.assertTrue(meta["etag"].startswith('"'))
        self.assertEqual(json.loads((self.tmp / (NAMES["catalog"] + ".meta.json")).read_text())["last_modified"], "Thu, 05 Nov 2026 22:30:56 GMT")
        self.assertEqual(f.json("catalog")["_meta"]["source"], "SendCutSend")
        self.assertTrue(net.calls[0].get_header("User-agent").startswith("Backplane"))

    def test_a_copy_within_the_ttl_is_used_without_asking(self):
        net, clock = FakeNet(), Clock()
        fetcher(self.tmp, net, clock).get("specs")
        clock.t += 600
        g = fetcher(self.tmp, net, clock).get("specs")
        self.assertEqual((g["how"], len(net.calls)), ("cached", 1))

    def test_an_old_copy_is_revalidated_by_etag_or_last_modified(self):
        net, clock = FakeNet(), Clock()
        f = fetcher(self.tmp, net, clock)
        f.get("guide"), f.get("catalog")
        clock.t += 7200
        f2 = fetcher(self.tmp, net, clock)
        self.assertEqual(f2.get("guide")["how"], "revalidated")
        self.assertIsNotNone(net.calls[-1].get_header("If-none-match"))
        self.assertEqual(f2.get("catalog")["how"], "revalidated")
        self.assertEqual(net.calls[-1].get_header("If-modified-since"), "Thu, 05 Nov 2026 22:30:56 GMT")
        meta = json.loads((self.tmp / (NAMES["guide"] + ".meta.json")).read_text())
        self.assertGreaterEqual(meta["checked_at"], clock.t - 1)
        self.assertLess(meta["fetched_at"], meta["checked_at"])

    def test_a_changed_file_replaces_the_copy(self):
        net, clock = FakeNet(), Clock()
        fetcher(self.tmp, net, clock).get("guide")
        net.bodies["guide"] = b"# SendCutSend\n\nnew words\n"
        clock.t += 7200
        g = fetcher(self.tmp, net, clock).get("guide")
        self.assertEqual((g["how"], g["data"]), ("downloaded", b"# SendCutSend\n\nnew words\n"))

    def test_offline_falls_back_to_the_copy_and_says_how_old(self):
        net, clock = FakeNet(), Clock()
        fetcher(self.tmp, net, clock).get("specs")
        clock.t += 3 * 86400
        net.down = True
        f = fetcher(self.tmp, net, clock)
        g = f.get("specs")
        self.assertEqual(g["how"], "stale")
        line = f.line("specs")
        self.assertIn("OFFLINE", line)
        self.assertIn("3.0 days old", line)
        self.assertIn("generated 2026-11-05", line)

    def test_no_copy_and_no_network_is_an_error(self):
        net = FakeNet()
        net.down = True
        with self.assertRaises(scs.ScsError) as e:
            fetcher(self.tmp, net).get("catalog")
        self.assertIn("no cached copy", str(e.exception))

    def test_a_broken_answer_never_replaces_a_good_copy(self):
        net, clock = FakeNet(), Clock()
        good = fetcher(self.tmp, net, clock).get("catalog")["data"]
        net.bodies["catalog"] = b"<html>maintenance</html>"
        clock.t += 7200
        g = fetcher(self.tmp, net, clock).get("catalog")
        self.assertEqual((g["how"], g["data"]), ("stale", good))

    def test_scs_offline_never_touches_the_network(self):
        net, clock = FakeNet(), Clock()
        fetcher(self.tmp, net, clock).get("guide")
        clock.t += 7200
        g = fetcher(self.tmp, net, clock, offline=True).get("guide")
        self.assertEqual((g["how"], len(net.calls)), ("stale", 1))

    def test_requests_are_a_second_apart(self):
        clock = Clock()
        f = fetcher(self.tmp, FakeNet(), clock)
        for n in scs.SOURCES:
            f.get(n)
        self.assertEqual(len(clock.slept), 2)
        self.assertTrue(all(0 < s <= 1.0 for s in clock.slept))


def run(m, data, sku, **kw):
    return {r["rule"]: r for r in scs.dfm(m, data, sku, **kw).rows}


class DfmTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = Path(tempfile.mkdtemp(prefix="scs-dfm-"))
        cls.data = scs.Data(seeded(cls.tmp / "cache"))

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls.tmp, ignore_errors=True)

    def dxf(self, name, **kw):
        p = self.tmp / name
        S.write_dxf(p, **kw)
        return scs.measure(p)

    def assertStatus(self, rows, rule, status):
        self.assertIn(rule, rows, f"no rule {rule}: {sorted(rows)}")
        self.assertEqual(rows[rule]["status"], status, rows[rule])

    # the bent bracket
    def test_the_sample_bracket_passes_every_rule(self):
        rep = scs.dfm(scs.measure(FIX / "bracket.dxf"), self.data, "ALU-063", finish=("powder", "matte_black"), hardware=["F-M3-1"])
        self.assertEqual(rep.count("fail"), 0, rep.text())
        rows = {r["rule"]: r for r in rep.rows}
        for rule in ("bend 1 flange", "bend 1 hole distance", "bend 1 spans", "min hole", "min web", "hole to edge", "max size", "powder size", "hardware F-M3-1 edge"):
            self.assertStatus(rows, rule, "pass")
        self.assertIn("bending_specs.min_flange_length_before_bend", rows["bend 1 flange"]["source"])
        self.assertIn("specs ALU-063", rows["min hole"]["source"])

    def test_a_hole_too_close_to_the_bend_fails(self):
        kw, _ = MF.bracket(hole_from_bend=2.0)
        rows = run(self.dxf("near.dxf", **kw), self.data, "ALU-063")
        self.assertStatus(rows, "bend 1 hole distance", "fail")

    def test_a_short_flange_fails(self):
        kw, _ = MF.bracket(up=5.0)
        kw["circles"] = kw["circles"][:2]
        rows = run(self.dxf("short.dxf", **kw), self.data, "ALU-063")
        self.assertStatus(rows, "bend 1 flange", "fail")

    def test_a_bend_line_that_stops_inside_the_part_fails(self):
        kw, _ = MF.bracket()
        (a, b), = kw["bends"]
        kw["bends"] = [((a[0] + 20, a[1]), b)]
        rows = run(self.dxf("partial.dxf", **kw), self.data, "ALU-063")
        self.assertStatus(rows, "bend 1 spans", "fail")

    def test_a_too_steep_bend_fails(self):
        rows = run(scs.measure(FIX / "bracket.dxf"), self.data, "ALU-063", bend_angles=[175])
        self.assertStatus(rows, "bend 1 angle", "fail")

    def test_a_small_part_is_too_small_to_powder_coat(self):
        kw, _ = MF.bracket()
        kw["outline"] = [(0, 0), (40, 0), (40, kw["outline"][2][1]), (0, kw["outline"][2][1])]
        kw["circles"] = [(20, 10, 2)]
        kw["bends"] = [((0, kw["bends"][0][0][1]), (40, kw["bends"][0][0][1]))]
        rows = run(self.dxf("small.dxf", **kw), self.data, "ALU-063", finish=("powder", "matte_black"))
        self.assertStatus(rows, "powder size", "fail")

    def test_mdf_is_not_bent(self):
        rows = run(scs.measure(FIX / "bracket.dxf"), self.data, "MDF-250")
        self.assertStatus(rows, "service bending", "fail")
        self.assertStatus(rows, "bending", "fail")

    def test_the_limits_come_from_the_specs_not_the_code(self):
        def edit(o):
            for m in o["materials"]:
                if m["sku"] == "ALU-063":
                    m["cutting_specs"]["min_hole_size"] = "0.200"
        data = scs.Data(seeded(self.tmp / "cache-edited", edit))
        rows = run(scs.measure(FIX / "bracket.dxf"), data, "ALU-063")
        self.assertStatus(rows, "min hole", "fail")
        self.assertIn('0.200"', rows["min hole"]["text"])

    # the MDF panel with cutouts
    def test_the_sample_mdf_panel_passes_every_rule(self):
        rep = scs.dfm(scs.measure(FIX / "mdf_panel.dxf"), self.data, "MDF-250")
        self.assertEqual(rep.count("fail"), 0, rep.text())
        rows = {r["rule"]: r for r in rep.rows}
        for rule in ("min hole", "min slot", "min web", "hole to edge", "closed contours", "units", "max size"):
            self.assertStatus(rows, rule, "pass")
        m = scs.measure(FIX / "mdf_panel.dxf")
        self.assertEqual(sorted(lp["shape"] for lp in m["inner"]), ["cutout", "cutout", "hole", "hole", "hole", "hole", "hole", "slot", "slot"])

    def panel(self, **change):
        kw = MF.mdf_panel()
        kw.update(change)
        return kw

    def test_a_thin_web_between_cutouts_fails(self):
        kw = self.panel()
        kw["circles"] = kw["circles"] + [(100 + 11 + 1.0 + 3, 30, 3)]  # 1 mm from the big hole
        rows = run(self.dxf("web.dxf", **kw), self.data, "MDF-250")
        self.assertStatus(rows, "min web", "fail")

    def test_a_hole_by_the_edge_fails(self):
        kw = self.panel()
        kw["circles"] = kw["circles"] + [(150, 200 - 3 - 1, 3)]
        rows = run(self.dxf("edge.dxf", **kw), self.data, "MDF-250")
        self.assertStatus(rows, "hole to edge", "fail")

    def test_a_narrow_slot_and_a_small_hole_fail(self):
        kw = self.panel(loops=MF.mdf_panel()["loops"] + [S.slot((200, 100), 20, 2, 0)], circles=MF.mdf_panel()["circles"] + [(150, 170, 0.8)])
        rows = run(self.dxf("narrow.dxf", **kw), self.data, "MDF-250")
        self.assertStatus(rows, "min slot", "fail")
        self.assertStatus(rows, "min hole", "fail")

    def test_text_open_paths_and_missing_units_fail(self):
        p = self.tmp / "bad.dxf"
        t = S.dxf_text(**self.panel())
        t = t.replace("$INSUNITS\n70\n4\n", "$INSUNITS\n70\n0\n")
        t = t.replace("0\nENDSEC\n0\nEOF\n", "0\nTEXT\n8\n0\n10\n5.0\n20\n5.0\n40\n3.0\n1\nHELLO\n0\nLINE\n8\n0\n10\n50.0\n20\n170.0\n11\n80.0\n21\n180.0\n0\nENDSEC\n0\nEOF\n")
        p.write_text(t)
        rows = run(scs.measure(p), self.data, "MDF-250")
        for rule in ("units", "no text", "closed contours"):
            self.assertStatus(rows, rule, "fail")

    def test_a_shape_inside_a_cutout_is_part_in_part(self):
        kw = self.panel()
        kw["loops"] = kw["loops"] + [S.poly_loop([(60, 80), (80, 80), (80, 100), (60, 100)])]  # inside the display window
        rows = run(self.dxf("pip.dxf", **kw), self.data, "MDF-250")
        self.assertStatus(rows, "part-in-part", "warn")

    def test_an_unknown_material_says_so(self):
        with self.assertRaises(scs.ScsError) as e:
            self.data.material("ALU-064")
        self.assertIn("ALU-063", str(e.exception))

    # the fab file
    def fab(self, part, sku, *extra):
        out = self.tmp / (Path(part).stem + ".fab.json")
        a = ["fab", str(part), "--sku", sku, "-o", str(out), *extra]
        env = dict(os.environ, SCS_CACHE=str(self.tmp / "cache"), SCS_OFFLINE="1")
        r = subprocess.run([sys.executable, "-I", str(SKILL / "scripts/scs.py"), *a], env=env, capture_output=True, text=True)
        return r, json.loads(out.read_text()) if out.exists() else None

    def test_the_bracket_gets_a_sheetmetal_fab_file_backplane_can_price(self):
        shutil.copy(FIX / "bracket.dxf", self.tmp / "bracket.dxf")
        r, fab = self.fab(self.tmp / "bracket.dxf", "ALU-063", "--finish", "powder:matte_black", "--hardware", "F-M3-1", "--qty", "4")
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertRegex(r.stdout, r"sendcutsend-specs.json: fetched .* ago")  # it says how old its copy is
        self.assertEqual((fab["kind"], fab["qty"], fab["source"]), ("sheetmetal", 4, "bracket.dxf"))
        self.assertEqual(fab["options"], {"material": "aluminum_5052", "thickness_mm": 1.6, "finish": "powder_coat"})
        f = fab["features"]
        self.assertEqual((f["holes"], f["bends"], f["flat_x_mm"]), (3, 1, 80.0))
        self.assertAlmostEqual(f["flat_y_mm"], 67.47, places=1)
        self.assertGreater(f["cut_length_mm"], 2 * (80 + 67))
        s = fab["sendcutsend"]
        self.assertEqual((s["sku"], s["unmapped"], s["dfm"]["fail"]), ("ALU-063", [], 0))
        self.assertEqual(set(s["services"]), {"powder", "bending", "hardware"})
        self.assertEqual(s["dfm"]["source_sha256"], scs.sha256(self.tmp / "bracket.dxf"))
        # and the fab file alone is enough to check it again
        env = dict(os.environ, SCS_CACHE=str(self.tmp / "cache"), SCS_OFFLINE="1")
        r2 = subprocess.run([sys.executable, "-I", str(SKILL / "scripts/scs.py"), "dfm", str(self.tmp / "bracket.fab.json"), "--json"], env=env, capture_output=True, text=True)
        self.assertEqual(r2.returncode, 0, r2.stderr)
        self.assertEqual(json.loads(r2.stdout)["fail"], 0)

    def test_the_mdf_panel_is_a_router_part_and_its_material_is_named_unmapped(self):
        shutil.copy(FIX / "mdf_panel.dxf", self.tmp / "mdf_panel.dxf")
        r, fab = self.fab(self.tmp / "mdf_panel.dxf", "MDF-250")
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertEqual(fab["kind"], "cnc")
        self.assertNotIn("material", fab["options"])
        self.assertEqual(fab["sendcutsend"]["unmapped"], ["material MDF"])
        self.assertEqual((fab["features"]["size_x_mm"], fab["features"]["size_y_mm"], fab["features"]["size_z_mm"]), (300.0, 200.0, 6.35))

    def test_a_failing_part_exits_1_and_its_fab_file_lists_the_failures(self):
        kw, _ = MF.bracket(hole_from_bend=2.0)
        S.write_dxf(self.tmp / "nearfab.dxf", **kw)
        r, fab = self.fab(self.tmp / "nearfab.dxf", "ALU-063")
        self.assertEqual(r.returncode, 1)
        self.assertTrue(any(x.startswith("bend 1 hole distance") for x in fab["sendcutsend"]["dfm"]["failed"]))


def freecad():
    for c in (os.environ.get("BACKPLANE_FREECAD"), shutil.which("freecadcmd"), os.path.expanduser("~/.local/bin/freecadcmd")):
        if c and os.path.isfile(c):
            return c
    return None


@unittest.skipUnless(freecad(), "FreeCAD's freecadcmd is not installed")
class StepTests(unittest.TestCase):
    def test_the_example_bracket_step_is_measured_and_passes(self):
        tmp = Path(tempfile.mkdtemp(prefix="scs-step-"))
        try:
            seeded(tmp / "cache")
            env = dict(os.environ, SCS_CACHE=str(tmp / "cache"), SCS_OFFLINE="1", SCS_SKILL=str(SKILL / "scripts"), PART_OUT=str(tmp), BACKPLANE_FREECAD=freecad())
            code = f"exec(open({str(SKILL / 'examples/bracket.py')!r}).read())"
            r = subprocess.run([freecad(), "-c", code], env=env, capture_output=True, text=True, timeout=300, cwd=tmp)
            self.assertTrue((tmp / "bracket.step").is_file(), r.stdout[-800:] + r.stderr[-800:])
            os.environ.update(SCS_CACHE=str(tmp / "cache"), SCS_OFFLINE="1", BACKPLANE_FREECAD=freecad())
            f = scs.Fetcher()
            m = scs.measure(tmp / "bracket.step")
            st = m["step"]
            self.assertAlmostEqual(st["thickness"], 0.063, places=3)
            self.assertEqual(len(st["bends"]), 1)
            self.assertAlmostEqual(st["bends"][0]["radius"], 0.035, places=3)
            self.assertAlmostEqual(st["bends"][0]["angle"], 90, places=1)
            self.assertEqual(len(st["holes"]), 3)
            rep = scs.dfm(m, scs.Data(f), "ALU-063", hardware=["F-M3-1"], flat=scs.measure(tmp / "bracket.flat.dxf"))
            self.assertEqual(rep.count("fail"), 0, rep.text())
            rows = {x["rule"]: x for x in rep.rows}
            self.assertEqual(rows["thickness"]["status"], "pass")
            self.assertEqual(rows["bend 1 radius"]["status"], "pass")
        finally:
            for k in ("SCS_CACHE", "SCS_OFFLINE"):
                os.environ.pop(k, None)
            shutil.rmtree(tmp, ignore_errors=True)


@unittest.skipUnless(os.environ.get("SCS_LIVE") == "1", "the live fetch runs with SCS_LIVE=1")
class LiveTests(unittest.TestCase):
    def test_the_three_live_files_fetch_parse_and_revalidate(self):
        tmp = Path(tempfile.mkdtemp(prefix="scs-live-"))
        try:
            f = scs.Fetcher(cache=tmp, ttl_s=0)
            for n in scs.SOURCES:
                g = f.get(n)
                self.assertEqual(g["how"], "downloaded", n)
                self.assertEqual(scs.valid(n, g["data"]), "", n)
            data = scs.Data(f)
            self.assertGreater(len(data.mats), 50)
            self.assertTrue(all(k in data.spec for k in list(data.mats)[:20]))
            f2 = scs.Fetcher(cache=tmp, ttl_s=0)
            hows = {n: f2.get(n)["how"] for n in scs.SOURCES}
            self.assertTrue(all(h in ("revalidated", "downloaded") for h in hows.values()), hows)
            print("\nlive:", "; ".join(f2.line(n) for n in scs.SOURCES), hows, file=sys.stderr)
        finally:
            shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    unittest.main()
