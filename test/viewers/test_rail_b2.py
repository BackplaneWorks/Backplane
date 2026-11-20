import tempfile
import unittest
from pathlib import Path

from PIL import Image, ImageDraw

from rail_b2 import IDENTITY, TOOL_ORDER, identity_font, measure, tool_orders


def rect(x, y, w, h):
    return dict(x=x, y=y, w=w, h=h)


class HeaderChecks(unittest.TestCase):
    def test_scaled_row_and_actual_identity_ink(self):
        text = "Board · " + IDENTITY["Board"]
        image = Image.new("RGB", (900, 140))
        draw = ImageDraw.Draw(image)
        draw.text((166, 32), text, fill="white", font=identity_font(18))
        draw.text((785, 32), "×", fill="white", font=identity_font(18))
        probes = {"pane-row-reference": rect(0, 20, 160, 48),
                  "pane-header:board": rect(160, 20, 650, 48),
                  "pane-identity:board:" + text: rect(164, 22, 300, 44),
                  "pane-tools:board": rect(480, 22, 30, 30),
                  "pane-close:board": rect(780, 22, 28, 42)}
        shot = {"probe_rects": probes, "settled": True, "viewer": {"open": "board"},
                "layout": {"main": rect(0, 20, 160, 100), "rail": rect(810, 20, 90, 100),
                           "topbar": rect(0, 0, 900, 20)}}
        with tempfile.TemporaryDirectory() as folder:
            crop = Path(folder) / "header.png"
            self.assertTrue(measure(image, shot, "Board", "wide", crop)["checks"]["identity_painted"])
            draw.rectangle((164, 22, 464, 66), fill="black")
            draw.text((166, 32), text.replace("ecc83", "xxxxx"), fill="white", font=identity_font(18))
            self.assertFalse(measure(image, shot, "Board", "wide", crop)["checks"]["identity_painted"])
            probes["pane-row-reference"]["h"] = 32
            del probes["pane-close:board"]
            result = measure(image, shot, "Board", "wide", crop)
            self.assertFalse(result["checks"]["row_height"])
            self.assertFalse(result["checks"]["close_painted"])

    def test_missing_suffix_requires_overflow_and_ordered_prefix(self):
        reports = {size: {} for size in ("wide", "minimum")}
        for page, schema in TOOL_ORDER.items():
            ids = [i for i in schema if not i.startswith("?")]
            if page in ("board", "schematic"):
                ids.extend(["follow-agent", "kicad-edit"])
            if page == "split":
                ids.append("follow-agent")
            for size in reports:
                reports[size][page] = dict(checks={}, outcome="passed", tools=ids[:], more=False)
        reports["minimum"]["board"]["tools"] = []
        reports["minimum"]["schematic"].update(tools=["plot-fit-0"], more=True,
            menu_rows=["layers-btn", "follow-agent", "kicad-edit"])
        reports["minimum"]["review"].update(tools=["rv-0"], more=True, menu_rows=["rv-1", "rv-2", "rv-3"])
        reports["minimum"]["diff"].update(tools=[], more=True, menu_rows=["revert-ask"])
        reports["wide"]["files"]["tools"].reverse()
        tool_orders(reports)
        self.assertEqual(reports["minimum"]["board"]["outcome"], "task_failed")
        self.assertEqual(reports["minimum"]["schematic"]["outcome"], "passed")
        self.assertEqual(reports["minimum"]["review"]["outcome"], "passed")
        self.assertEqual(reports["wide"]["files"]["outcome"], "task_failed")
        self.assertEqual(reports["minimum"]["diff"]["outcome"], "task_failed")
