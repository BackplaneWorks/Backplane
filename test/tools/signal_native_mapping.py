#!/usr/bin/env python3
"""Compile the actual indicator mapping, isolated from SwiftUI/Compose."""

import argparse
import json
import pathlib
import subprocess


def function(text, marker):
    start = text.index(marker)
    opening = text.index("{", start)
    depth = 1
    end = opening + 1
    while depth:
        depth += (text[end] == "{") - (text[end] == "}")
        end += 1
    return text[start:end]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", default="build/stoplight/native-mapping")
    parser.add_argument("--kotlin-lib", default="/opt/gradle/gradle-8.14.3/lib")
    parser.add_argument("--generate-only", action="store_true")
    parser.add_argument("--java", default="/usr/lib/jvm/java-17-openjdk-amd64/bin/java")
    args = parser.parse_args()
    root = pathlib.Path(__file__).resolve().parents[2]
    out = (root / args.out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    swift = (root / "mobile/ios/Backplane/ThreadParts.swift").read_text()
    kotlin = (root / "mobile/android/app/src/main/java/dev/backplane/mobile/ThreadParts.kt").read_text()
    expected = {
        "blocked": "blocked", "approval": "blocked", "input": "blocked",
        "needs you": "blocked", "failed": "blocked",
        "active": "active", "running": "active", "watching": "active",
        "cancelling": "active", "working": "active", "monitoring": "active",
        "settled": "settled", "done": "settled", "completed": "settled", "cancelled": "settled",
        "inactive": "inactive", "queued": "inactive", "waiting": "inactive",
        "finished": "finished", "complete": "finished",
        "stopped": "inactive", "idle": "inactive", "ready": "inactive",
        "held": "inactive", "reporting": "inactive", "unknown": "inactive",
        "launching": "inactive", "future-status": "inactive",
    }
    cases = [(state, status, result) for state in ["", "run", "fail", "stop", "done"]
             for status, result in expected.items()]
    cases += [(state, "", result) for state, result in
              [("run", "active"), ("fail", "blocked"), ("stop", "inactive"), ("done", "inactive"), ("", "inactive")]]
    (out / "cases.json").write_text(json.dumps(cases, indent=2) + "\n")
    sf = function(swift, "private func threadStatusSignal(")
    kf = function(kotlin, "private fun threadStatusSignal(")
    swift_cases = ",\n".join("(" + ",".join(json.dumps(v) for v in c) + ")" for c in cases)
    (out / "SignalMapping.swift").write_text(sf + "\nlet cases: [(String,String?,String)] = [\n" + swift_cases + "]\n"
        + 'for (state,status,want) in cases { precondition(threadStatusSignal(state,status) == want, "mapping: \\(state)/\\(status ?? "nil")") }\n'
        + 'precondition(threadStatusSignal("run",nil) == "active")\n'
        + 'precondition(threadStatusSignal("fail",nil) == "blocked")\n'
        + f'print("PASS Swift actual source mapping {len(cases) + 2} cases")\n')
    kotlin_cases = ",\n".join("Triple(" + ",".join(json.dumps(v) for v in c) + ")" for c in cases)
    (out / "SignalMapping.kt").write_text(kf + "\nfun main() {\nval cases = listOf(\n" + kotlin_cases + ")\n"
        + 'for ((state,status,want) in cases) { check(threadStatusSignal(state,status) == want) { "mapping: $state/$status" } }\n'
        + f'println("PASS Kotlin actual source mapping {len(cases)} cases")\n}}\n')
    if not args.generate_only:
        lib = pathlib.Path(args.kotlin_lib)
        standard = str(next(lib.glob("kotlin-stdlib-*.jar")))
        subprocess.run([args.java, "-cp", str(lib / "*"), "org.jetbrains.kotlin.cli.jvm.K2JVMCompiler",
                        "-no-stdlib", "-no-reflect", "-classpath", standard,
                        str(out / "SignalMapping.kt"), "-d", str(out / "mapping.jar")], check=True)
        subprocess.run([args.java, "-cp", str(out / "mapping.jar") + ":" + standard,
                        "SignalMappingKt"], check=True)
    print("Swift standalone fixture:", out / "SignalMapping.swift")


if __name__ == "__main__":
    main()
