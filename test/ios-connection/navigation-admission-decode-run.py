#!/usr/bin/env python3
"""Compile actual Screen/Out and its data models, without UI framework views.

Models embedded in SwiftUI/ActivityKit files are copied verbatim from bounded
declarations. This checks decoding only, not those frameworks or an app build.
"""
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[2]
with tempfile.TemporaryDirectory(prefix="bp-ios-navigation-admission-decode-") as work:
    binary = Path(work) / "checks"

    def section(name, start, end):
        text = (root / name).read_text()
        assert text.count(start) == 1 and text.count(end) == 1, name
        first = text.index(start)
        last = text.index(end, first)
        return text[first:last]

    models = Path(work) / "ScreenModelDependencies.swift"
    island = (root / "mobile/ios/Shared/IslandAttributes.swift").read_text()
    # Keep the real nested Codable payload declarations. ActivityAttributes
    # itself is not needed to decode the Screen's ContentState value.
    assert island.count("    struct Line: Codable, Hashable {") == 1
    models.write_text("import Foundation\n" + "\n".join([
        section("mobile/ios/Backplane/SpaceView.swift", "struct SpaceModel:", "struct SpaceView:"),
        section("mobile/ios/Backplane/SpaceView.swift", "struct SpacePageModel:", "struct SpacePage:"),
        section("mobile/ios/Backplane/Cat.swift", "struct CatAnim:", "func catRig("),
        section("mobile/ios/Backplane/BubMove.swift", "struct BubMove:", "struct BubMoveOverlay:"),
        section("mobile/ios/Backplane/Plane.swift", "struct UberCard:", "private func planeStatus("),
        "enum IslandAttributes {\n" + island[island.index("    struct Line: Codable, Hashable {"):],
    ]))
    subprocess.run(["swiftc", "-parse-as-library",
                    str(root / "mobile/ios/Backplane/Screen.swift"),
                    str(root / "mobile/ios/Backplane/MailroomAction.swift"),
                    str(root / "mobile/ios/Backplane/NavigationPresentationReadiness.swift"),
                    str(root / "mobile/ios/Backplane/MessagePartial.swift"),
                    str(models),
                    str(root / "mobile/ios/Backplane/WireDecodeDiagnostic.swift"),
                    str(root / "test/ios-connection/navigation-admission-decode.swift"),
                    "-o", str(binary)], check=True)
    subprocess.run([str(binary)], check=True, timeout=10)
