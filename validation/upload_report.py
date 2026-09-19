#!/usr/bin/env python3
"""voxelcraft upload report.

Runs the release binary against a baseline capture, then sweeps every CLI
flag, comparing each variant's screenshot against the baseline with the
binary's own --diff (MAE / differing-pixel count), benchmarks the
perf-relevant flags with --bench-frames, exercises the functional paths
(cargo test, --bench-terrain, --lookbook, edit-journal replay), and writes
everything to uploadme.txt next to this run. Send that file back when asked;
screenshots are kept in uploadme_runs/ for reference.

Arms that may move zero pixels because the baseline capture has nothing for
the feature to touch are paired with a curated vantage (from the same
catalogue src/harness/vantage.rs uses) or given the INERT expectation, so a
zero-pixel arm stays a signal instead of noise. Paired arms are diffed
against a cached CONTEXT baseline -- the same vantage with the arm's flag
left off -- so the measured pixels are the flag's alone and an arm can never
pass by merely pointing the camera elsewhere.

Usage:
    python3 validation/upload_report.py [--fast] [--skip-tests] [--skip-lookbook]
                                        [--skip-perf] [--cases SUBSTR]
                                        [--out uploadme.txt] [--runs-dir DIR]

Exit code is 0 when no FAIL verdicts occurred, 1 otherwise (SUSPECT does not
fail the run -- it flags arms that moved no pixels, which may be scene-side
rather than broken).
"""

import argparse
import os
import platform
import re
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# ---------------------------------------------------------------------------
# Run configuration
# ---------------------------------------------------------------------------

class Knobs:
    def __init__(self, fast):
        self.width = 640 if fast else 960
        self.height = 360 if fast else 540
        self.taa = 6 if fast else 12          # uniform across every capture
        self.bench_frames = 90 if fast else 150
        self.terrain_chunks = 24 if fast else 48
        self.shot_timeout = 360
        self.bench_timeout = 420
        self.cargo_test_timeout = 2400
        self.lookbook_timeout = 1800

# Flag sweep matrix. Each case: (id, extra args, expectation).
#   EXACT  -- args merely restate defaults; diff must be zero (catches drift
#             between documented defaults and actual defaults).
#   CHANGE -- args must move pixels vs baseline (catches dead arms).
UN = "EXACT"
CH = "CHANGE"
INERT = "may-be-inert"

CASES = [
    # -- restated defaults: must be bit-identical to baseline ---------------
    ("default-grade",            ["--grade", "none"],                                         UN),
    ("tone-map-knee",            ["--tone-map", "knee"],                                      UN),
    ("sun-softness-normal",      ["--sun-softness", "normal"],                                UN),
    ("surface-debug-off",        ["--surface-debug", "off"],                                  UN),
    ("repaired-lighting",        ["--repaired-lighting"],                                     UN),
    ("repaired-water",           ["--repaired-water"],                                        UN),
    ("no-soft-shadows",          ["--no-soft-shadows"],                                       UN),
    ("no-soft-shadow-hq",        ["--no-soft-shadow-hq"],                                     UN),
    ("shadow-dist-220",           ["--shadow-dist", "220"],                                    UN),
    ("no-wind-sway",             ["--no-wind-sway"],                                          UN),
    ("no-water-look",            ["--no-water-look"],                                         UN),
    ("no-glass-reflect",         ["--no-glass-reflect"],                                      UN),
    ("no-isolate-glass",         ["--no-isolate-glass"],                                      UN),
    ("exp-shadow-repro",        ["--exp-shadow-repro"],                                      UN),
    ("exp-temporal-hiz",        ["--exp-temporal-hiz"],                                      UN),

    ("no-compact-shade-hit",     ["--no-compact-shade-hit"],                                  UN),
    ("compact-shade-hit",        ["--compact-shade-hit"],                                     UN),
    ("water-sec-scale-2",        ["--water-sec-scale", "2"],                                  UN),
    ("fade-schedule-smoothstep", ["--fade-schedule", "smoothstep"],                           UN),
    ("cam-defaults",             ["--cam-height", "40", "--cam-yaw", "40", "--cam-pitch", "-14"], UN),
    ("lod-defaults",             ["--view-distance", "2600", "--streaming-factor", "2.0",
                                  "--fade-band", "1.25", "--max-lod", "4"],                   UN),
    ("seed-sea-defaults",        ["--seed", "1337", "--sea-level", "128"],                    UN),
    ("time-0.30",                ["--time", "0.30"],                                          UN),
    ("fog-defaults",             ["--fog-density", "0.0005", "--fog-falloff", "0.015625",
                                  "--fog-scatter", "0.02", "--fog-g", "0.8"],                 UN),
    ("wave-defaults",            ["--wave-amp", "0.12", "--wave-scale", "32",
                                  "--wave-speed", "0.32", "--wave-clamp", "0"],               UN),
    ("grade-strength-1",         ["--grade-strength", "1.0"],                                 UN),
    ("tint-strength-1",          ["--tint-strength", "1.0"],                                  UN),
    ("res-defaults",             ["--mip-bias", "0", "--scale", "1.0"],                       UN),
    ("hotbar-4",                 ["--hotbar", "4"],                                           UN),
    ("shadow-pass-explicit",     ["--shadow-pass"],                                           UN),
    ("no-shadow-pass-retired",   ["--no-shadow-pass"],                                        UN),
    ("ambient-0.08",             ["--ambient", "0.08"],                                       UN),
    ("godray-defaults",          ["--godray-strength", "1.0", "--godray-steps", "4",
                                  "--godray-dist", "4096", "--cloud-shadow", "0.85"],         UN),
    ("fov-70",                   ["--fov", "70"],                                             UN),
    ("shaft-texel-4",            ["--shaft-texel", "4"],                                      UN),
    ("water-mottle-stub",        ["--water-mottle", "0.2"],                                   UN),
    ("bar-default",              ["--bar", "1,2,3,4,8,6,7,9,10,11"],                          UN),
]

# -- arms that must move pixels ----------------------------------------------
CASES += [
    ("no-water",                 ["--no-water"],                                              CH),
    ("no-waves",                 ["--no-waves"],                                              CH),
    ("no-water-reflect",         ["--no-water-reflect"],                                      CH),
    ("no-water-refract",         ["--no-water-refract"],                                      CH),
    ("no-shore-wet",             ["--no-shore-wet"],                                          CH),
    ("no-shore-foam",            ["--no-shore-foam"],                                         CH),
    ("no-water-sec",             ["--no-water-sec"],                                          INERT),
    ("water-sec-scale-1",        ["--water-sec-scale", "1"],                                  CH),
    ("water-sec-scale-4",        ["--water-sec-scale", "4"],                                  INERT),
    ("water-absorb-3",           ["--water-absorb", "3"],                                     CH),
    ("water-look",               ["--water-look"],                                            CH),
    ("no-water-far",           ["--no-water-far", "--cam-height", "60", "--cam-yaw", "120", "--cam-pitch", "-6"],  INERT),
    ("no-water-dark",          ["--no-water-dark", "--cam-submerge", "8"],               INERT),
    ("no-wave-aniso",            ["--no-wave-aniso"],                                         CH),
    ("no-wave-shoal",            ["--no-wave-shoal"],                                         CH),
    ("no-wave-fill",             ["--no-wave-fill"],                                          CH),
    ("wave-strong",              ["--wave-amp", "0.3", "--wave-scale", "12",
                                  "--wave-speed", "1.2"],                                     CH),
    ("snell-bend",             ["--snell-bend", "--cam-submerge", "5", "--cam-yaw", "143", "--cam-pitch", "30"],  CH),
    ("no-snell",               ["--no-snell", "--cam-submerge", "5", "--cam-yaw", "143", "--cam-pitch", "30"],  CH),
    ("caustics",                 ["--caustics"],                                              CH),
    ("cam-submerge",             ["--cam-submerge", "3"],                                     CH),
    ("no-light-rgb",           ["--no-light-rgb", "--demo-lamps", "--time", "0.30", "--cam-height", "-20", "--cam-yaw", "0", "--cam-pitch", "0"],  CH),
    ("no-ao",                    ["--no-ao"],                                                 CH),
    ("no-probe-shadow",          ["--no-probe-shadow"],                                       CH),
    ("no-probe-sun",             ["--no-probe-sun"],                                          CH),
    ("probe-sun-high",           ["--probe-sun-high"],                                        CH),
    ("no-probe-cube",            ["--no-probe-cube"],                                         CH),
    ("no-probe-bounce",          ["--no-probe-bounce"],                                       CH),
    ("probe-tap",                ["--probe-tap"],                                             CH),
    ("probe-ambient",            ["--probe-ambient"],                                         CH),
    ("probe-noise",            ["--probe-noise", "--probe-fill", "0.65"],                CH),
    ("probe-fill-4",             ["--probe-fill", "4"],                                       CH),
    ("ambient-0.2",              ["--ambient", "0.2"],                                        CH),
    ("no-sky-tint",              ["--no-sky-tint"],                                           CH),
    ("no-sky-specular",          ["--no-sky-specular"],                                       CH),
    ("sky-cool",                 ["--sky-cool"],                                              CH),
    ("cloud-cover-0.85",         ["--cloud-cover", "0.85"],                                   CH),
    ("cloud-cover-0",            ["--cloud-cover", "0"],                                      CH),
    ("cloud-height-400",         ["--cloud-height", "400"],                                   CH),
    ("cloud-patch-0.8",          ["--cloud-patch", "0.8"],                                    CH),
    ("cloud-relief-0.8",         ["--cloud-relief", "0.8"],                                   CH),
    ("haze-warm-0.8",            ["--haze-warm", "0.8"],                                      CH),
    ("zenith-deep-0.8",          ["--zenith-deep", "0.8"],                                    CH),
    ("fog-dense",                ["--fog-density", "0.004"],                                  CH),
    ("fog-isotropic",            ["--fog-g", "0.2"],                                          CH),
    ("fog-fast-falloff",         ["--fog-falloff", "0.03"],                                   CH),
    ("no-biomes",                ["--no-biomes"],                                             CH),
    ("no-foliage",               ["--no-foliage"],                                            CH),
    ("no-meadow",                ["--no-meadow"],                                             CH),
    ("no-tint",                  ["--no-tint"],                                               CH),
    ("tint-half",               ["--tint-strength", "0.5"],                                  CH),
    ("tint-balance",             ["--tint-balance"],                                          CH),
    ("foliage-rich",             ["--foliage-rich"],                                          CH),
    ("canopy-relief",            ["--canopy-relief"],                                         CH),
    ("canopy-lift-6",            ["--canopy-lift", "6"],                                      CH),
    ("tree-blue-noise",          ["--tree-blue-noise"],                                       CH),
    ("meadow-side",              ["--meadow-side"],                                           CH),
    ("grass-dense",              ["--grass-dense"],                                           CH),
    ("wind-sway",                ["--wind-sway"],                                             CH),
    ("no-leaf-cutout",           ["--no-leaf-cutout"],                                        CH),
    ("no-leaf-thin",             ["--no-leaf-thin"],                                          CH),
    ("no-tex-variation",         ["--no-tex-variation"],                                      CH),
    ("no-glass",               ["--no-glass", "--demo-glass", "--cam-submerge", "1", "--cam-yaw", "143", "--cam-pitch", "-25"],  CH),
    ("glass-reflect",          ["--glass-reflect", "--demo-glass", "--cam-submerge", "1", "--cam-yaw", "143", "--cam-pitch", "-25"],  CH),
    ("isolate-glass",            ["--isolate-glass"],                                          UN),
    ("no-shadows",               ["--no-shadows"],                                            CH),
    ("soft-shadows",             ["--soft-shadows"],                                          CH),
    ("soft-shadows-hq",          ["--soft-shadows", "--soft-shadow-hq"],                      CH),
    ("sun-softness-narrow",    ["--sun-softness", "narrow", "--soft-shadows"],           INERT),
    ("sun-softness-wide",      ["--sun-softness", "wide", "--soft-shadows"],             INERT),
    ("no-distant-shadows",       ["--no-distant-shadows"],                                    CH),
    ("no-water-shadow-cut",      ["--no-water-shadow-cut"],                                   CH),
    ("no-terrain-shafts",        ["--no-terrain-shafts"],                                     CH),
    ("shaft-texel-16",           ["--shaft-texel", "16"],                                     CH),
    ("godray-weak",              ["--godray-strength", "0.35"],                               CH),
    ("godray-dense",             ["--godray-steps", "16"],                                    CH),
    ("cloud-shadow-0.2",         ["--cloud-shadow", "0.2"],                                   CH),
    ("surface-debug-normals",    ["--surface-debug", "normals"],                              CH),
    ("legacy-lighting",          ["--legacy-lighting"],                                       CH),
    ("legacy-water",             ["--legacy-water"],                                          CH),
    ("no-full-march",            ["--no-full-march"],                                         CH),
    ("no-flat-secondary",        ["--no-flat-secondary"],                                     CH),
    ("grade-warm",               ["--grade", "warm"],                                         CH),
    ("grade-cine",               ["--grade", "cine"],                                         CH),
    ("grade-warm-half",          ["--grade", "warm", "--grade-strength", "0.5"],              CH),
    ("tone-map-aces",            ["--tone-map", "aces"],                                      CH),
    ("no-taa",                   ["--no-taa"],                                                CH),
    ("mip-bias-1.5",             ["--mip-bias", "1.5"],                                       CH),
    ("scale-0.5",                ["--scale", "0.5"],                                          CH),
    ("fov-90",                   ["--fov", "90"],                                             CH),
    ("view-distance-900",        ["--view-distance", "900"],                                  CH),
    ("max-lod-2",                ["--max-lod", "2"],                                          CH),
    ("no-fade",                  ["--no-fade"],                                               CH),
    ("no-shadow-share",          ["--no-shadow-share"],                                       CH),
    ("no-offscreen-shadows",   ["--no-offscreen-shadows", "--cam-height", "8", "--cam-pitch", "0"],  INERT),
    ("demo-edits",             ["--demo-edits", "--cam-height", "14", "--cam-pitch", "-20"],  CH),
    ("demo-glass",             ["--demo-glass", "--cam-height", "14", "--cam-pitch", "-20"],  CH),
    ("demo-lamps",             ["--demo-lamps", "--time", "0.30", "--cam-height", "-20", "--cam-yaw", "0", "--cam-pitch", "0"],  CH),
    ("hud",                      ["--hud"],                                                   CH),
]

# CHANGE arms allowed to be inert *in this scene*: the feature exists but the
# baseline capture has nothing for it to touch. A zero-pixel arm there reports
# inert-ok instead of suspect; if one starts moving pixels, the note is kept.
INERT_WHY = {
    "no-water-far": "no water beyond WATER_FAR_DIST (128 blocks) in view, even from the paired horizon vantage",
    "no-water-dark": "no water deeper than WATER_DARK_DEPTH (15 blocks) in view, even with a submerged eye",
    "no-offscreen-shadows": "no off-screen occluders in reach, even from the paired hudged-against-the-hill vantage",
    "no-water-sec": "the share path needs enough flat-water spans in frame -- 960x540 engages it, the 640x360 fast capture does not",
    "water-sec-scale-4": "same flat-water-span threshold as no-water-sec",
    "sun-softness-narrow": "soft contact is resolution-bound (12 px at 960x540); small captures can under-cover it",
    "sun-softness-wide": "soft contact is resolution-bound (12 px at 960x540); small captures can under-cover it",
}

# Paired scene context: the flags below only do anything at a non-default
# vantage, so the sweep renders the CONTEXT once (the arm's own args with the
# distinguishing flag dropped) and diffs the arm against that cached baseline.
# Without this, a paired arm diffs against the default capture and measures
# camera motion instead of the flag.
CTX = {
    "snell-bend":           ["--cam-submerge", "5", "--cam-yaw", "143", "--cam-pitch", "30"],
    "no-snell":             ["--cam-submerge", "5", "--cam-yaw", "143", "--cam-pitch", "30"],
    "no-light-rgb":         ["--demo-lamps", "--time", "0.30", "--cam-height", "-20",
                             "--cam-yaw", "0", "--cam-pitch", "0"],
    "probe-noise":          ["--probe-fill", "0.65"],
    "no-glass":             ["--demo-glass", "--cam-submerge", "1", "--cam-yaw", "143",
                             "--cam-pitch", "-25"],
    "glass-reflect":        ["--demo-glass", "--cam-submerge", "1", "--cam-yaw", "143",
                             "--cam-pitch", "-25"],
    "sun-softness-narrow":  ["--soft-shadows"],
    "sun-softness-wide":    ["--soft-shadows"],
    "no-water-far":         ["--cam-height", "60", "--cam-yaw", "120", "--cam-pitch", "-6"],
    "no-water-dark":        ["--cam-submerge", "8"],
    "no-offscreen-shadows": ["--cam-height", "8", "--cam-pitch", "0"],
}

def _check_ctx():
    by_id = {c[0]: c[1] for c in CASES}
    for cid, ctx in CTX.items():
        assert cid in by_id, f"CTX names unknown case {cid!r}"
        assert by_id[cid][-len(ctx):] == ctx, \
            f"{cid}: args must END with their declared context {ctx!r} (flags first, context last)"
_check_ctx()

# Perf sweep: (id, args). Wall median + gpu total are compared to the
# baseline bench on the same machine, hi-z block second pass.
PERF_CASES = [
    ("no-shadows",        ["--no-shadows"]),
    ("soft-shadows",      ["--soft-shadows"]),
    ("no-water",          ["--no-water"]),
    ("no-foliage",        ["--no-foliage"]),
    ("no-taa",            ["--no-taa"]),
    ("max-lod-2",         ["--max-lod", "2"]),
    ("scale-0.5",         ["--scale", "0.5"]),
    ("scale-1.25",        ["--scale", "1.25"]),
    ("no-full-march",     ["--no-full-march"]),
    ("compact-shade-hit", ["--compact-shade-hit"]),
    ("no-water-sec",      ["--no-water-sec"]),
    ("water-sec-scale-4", ["--water-sec-scale", "4"]),
    ("no-light-rgb",      ["--no-light-rgb"]),
    ("no-flat-secondary", ["--no-flat-secondary"]),
    # shadow reach experiment (was hardcoded 220.0) and the effects-lab
    # toggles, benched at the vantage the demo builders are built for.
    ("shadow-dist-128",   ["--shadow-dist", "128"]),
    ("demo-glass-b",      ["--demo-glass", "--cam-height", "14", "--cam-pitch", "-20"]),
    ("glass-reflect-b",   ["--demo-glass", "--glass-reflect",
                           "--cam-height", "14", "--cam-pitch", "-20"]),
    ("exp-shadow-repro-b", ["--exp-shadow-repro"]),
    ("exp-temporal-hiz-b", ["--exp-temporal-hiz"]),
    ("isolate-glass-b",   ["--demo-glass", "--isolate-glass",
                           "--cam-height", "14", "--cam-pitch", "-20"]),
]

# ---------------------------------------------------------------------------
# Parsing helpers (formats printed by src/headless.rs and cargo)
# ---------------------------------------------------------------------------

RE_DIFF = re.compile(r"differing\s+(\d+)\s+of\s+(\d+)\s+MAE\s+([\d.eE+-]+)\s+max\s+(\d+)")
RE_BBOX = re.compile(r"^bbox\s+(.*)$", re.M)
RE_GPU_NAME = re.compile(r"^gpu:\s+(.+)$", re.M)
RE_BENCH_HEAD = re.compile(r"^---\s+(\d+)x(\d+)\s+hi-z\s+(\w+)\s+taa\s+(\w+)\s+(\d+)\s+chunks\s+---", re.M)
RE_WALL = re.compile(r"wall\s+median\s+([\d.]+)ms\s+\(\s*(\d+)\s*fps\)\s+p99\s+([\d.]+)ms")
RE_GPU_TOTAL = re.compile(r"gpu\s+total\s+([\d.]+)ms")
RE_WROTE = re.compile(r"^wrote\s+(\S+)", re.M)
RE_CHUNKS = re.compile(r"(\d+)\s+chunks\s+in\s+([\d.]+)s")
RE_TESTLINE = re.compile(r"test result:\s+(\w+)\.\s+(\d+)\s+passed;\s+(\d+)\s+failed")
RE_PAIRS = re.compile(r"pairs marched\s+(\d+)\s+deferred\s+(\d+)")


def find_binary():
    exe = "voxelcraft.exe" if platform.system() == "Windows" else "voxelcraft"
    return ROOT / "target" / "release" / exe


def run(cmd, timeout, log, cwd=ROOT, env_extra=None):
    """Run a command, capture everything, never throw."""
    log.write("$ " + " ".join(str(c) for c in cmd) + "\n")
    t0 = time.time()
    env = dict(os.environ)
    if env_extra:
        env.update(env_extra)
    try:
        p = subprocess.run(
            [str(c) for c in cmd],
            cwd=str(cwd),
            capture_output=True,
            text=True,
            timeout=timeout,
            env=env,
        )
        out, err, code = p.stdout or "", p.stderr or "", p.returncode
    except subprocess.TimeoutExpired:
        out, err, code = "", f"TIMEOUT after {timeout}s", -9
    except OSError as e:
        out, err, code = "", f"OS ERROR: {e}", -1
    dt = time.time() - t0
    log.write(f"    (exit {code}, {dt:.1f}s)\n")
    return code, out, err, dt


class Report:
    def __init__(self):
        self.sections = []      # (title, [lines])
        self.fails = []         # (area, case, why)
        self.suspects = []
        self.inert = []         # (area, case, why) -- expected no-op in this scene
        self.notes = []

    def add(self, title, lines):
        self.sections.append((title, lines))

    def line(self, title, text):
        for t, lines in self.sections:
            if t == title:
                lines.append(text)
                return
        self.sections.append((title, [text]))

    def render(self):
        out = []
        for title, lines in self.sections:
            out.append(f"\n===== {title} =====")
            out.extend(lines)
        out.append("\n===== VERDICT =====")
        if self.fails:
            out.append(f"FAIL ({len(self.fails)}):")
            for area, case, why in self.fails:
                out.append(f"  [{area}] {case}: {why}")
        if self.suspects:
            out.append(f"SUSPECT ({len(self.suspects)}):")
            for area, case, why in self.suspects:
                out.append(f"  [{area}] {case}: {why}")
        if self.inert:
            out.append(f"EXPECTED-INERT ({len(self.inert)}):")
            for area, case, why in self.inert:
                out.append(f"  [{area}] {case}: {why}")
        if not self.fails and not self.suspects:
            out.append("ALL CHECKS PASSED")
        out.append(f"notes: {len(self.notes)}")
        out.extend("  " + n for n in self.notes)
        out.append("\n===== END OF REPORT =====")
        out.append("Send this file back. The PNGs it references are in the run directory")
        out.append("named above; keep or delete as you like.")
        return "\n".join(out) + "\n"


def screenshot(binary, base_args, extra, path, k, log):
    cmd = [binary, "--screenshot", path, *base_args, *extra]
    code, out, err, dt = run(cmd, k.shot_timeout, log)
    ok = code == 0 and RE_WROTE.search(out) and Path(path).exists()
    return ok, code, out, err, dt


def diff_pair(binary, a, b, log, crop=None):
    cmd = [binary, "--diff", a, b, "--grid", "2,2"]
    if crop:
        cmd += ["--crop", crop]
    code, out, err, dt = run(cmd, 120, log)
    m = RE_DIFF.search(out)
    if not m:
        return None, code, out, err
    bbox = RE_BBOX.search(out)
    return {
        "differ": int(m.group(1)),
        "total": int(m.group(2)),
        "mae": float(m.group(3)),
        "max": int(m.group(4)),
        "bbox": bbox.group(1) if bbox else "none",
    }, code, out, err


def bench(binary, width, height, frames, extra, k, log):
    cmd = [binary, "--bench-frames", str(frames),
           "--width", str(width), "--height", str(height), *extra]
    code, out, err, dt = run(cmd, k.bench_timeout, log)
    gpu = RE_GPU_NAME.search(out)
    heads = RE_BENCH_HEAD.findall(out)
    walls = RE_WALL.findall(out)
    totals = RE_GPU_TOTAL.findall(out)
    pairs = RE_PAIRS.search(out)
    blocks = []
    for i, h in enumerate(heads):
        b = {"hiz": h[2], "chunks": int(h[4])}
        if i < len(walls):
            b.update({"wall_ms": float(walls[i][0]), "fps": int(walls[i][1]),
                      "p99_ms": float(walls[i][2])})
        if i < len(totals):
            b["gpu_ms"] = float(totals[i])
        blocks.append(b)
    res = {
        "adapter": gpu.group(1) if gpu else None,
        "blocks": blocks,
        "pairs": (int(pairs.group(1)), int(pairs.group(2))) if pairs else None,
        "secs": dt,
    }
    return (code == 0 and blocks), code, res, out, err


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--fast", action="store_true")
    ap.add_argument("--skip-tests", action="store_true")
    ap.add_argument("--skip-lookbook", action="store_true")
    ap.add_argument("--skip-perf", action="store_true")
    ap.add_argument("--cases", default="")
    ap.add_argument("--out", default="uploadme.txt")
    ap.add_argument("--runs-dir", default="uploadme_runs")
    args = ap.parse_args()

    k = Knobs(args.fast)
    runs = ROOT / args.runs_dir
    runs.mkdir(exist_ok=True)
    rep = Report()
    runlog = open(ROOT / args.out + ".log", "w")   # full command trace

    W, H = k.width, k.height
    base_args = ["--seed", "1337", "--width", str(W), "--height", str(H),
                 "--taa-frames", str(k.taa)]

    # ---------------- environment ----------------
    env_lines = [
        f"date (UTC):   {datetime.now(timezone.utc).strftime('%Y-%m-%d %H:%M:%S')}",
        f"host:         {platform.platform()}",
        f"machine:      {platform.machine()}, cpus: {os.cpu_count()}",
        f"python:       {platform.python_version()}",
        f"repo:         {ROOT}",
        f"run dir:      {runs}",
        f"capture:      {W}x{H}, taa {k.taa}, seed 1337 (uniform pins)",
    ]
    for label, cmd in [("rustc", ["rustc", "--version"]), ("cargo", ["cargo", "--version"])]:
        code, out, err, _ = run(cmd, 60, runlog)
        env_lines.append(f"{label}:        {out.strip() if code == 0 else 'not found'}")
    code, out, err, _ = run(["git", "rev-parse", "--short", "HEAD"], 30, runlog)
    env_lines.append(f"git HEAD:     {out.strip() if code == 0 else 'unknown'}")
    code, out, err, _ = run(["git", "status", "--porcelain"], 30, runlog)
    env_lines.append(f"git dirty:    {'yes' if out.strip() else 'no'}")
    rep.add("ENVIRONMENT", env_lines)

    # ---------------- build ----------------
    t0 = time.time()
    code, out, err, _ = run(["cargo", "build", "--release"], 1800, runlog)
    build_secs = time.time() - t0
    warns = len(re.findall(r"\bwarning\b", out + err))
    rep.add("BUILD", [
        f"cargo build --release: exit {code} in {build_secs:.0f}s, {warns} warning-ish lines",
    ])
    if code != 0:
        rep.fails.append(("build", "cargo build --release", f"exit {code}"))
        rep.line("BUILD", "stderr tail:\n" + "\n".join((out + "\n" + err).splitlines()[-40:]))
        Path(ROOT / args.out).write_text(rep.render())
        runlog.close()
        print(f"build failed; report written to {args.out}")
        return 1
    binary = find_binary()
    if not binary.exists():
        rep.fails.append(("build", "binary", f"missing {binary}"))
        Path(ROOT / args.out).write_text(rep.render())
        runlog.close()
        return 1

    # ---------------- smoke: help + self diff ----------------
    code, out, err, _ = run([binary, "--help"], 60, runlog)
    smoke = [f"--help exits {code} ({len(out.splitlines())} lines of usage)"]
    if code != 0:
        rep.fails.append(("smoke", "--help", f"exit {code}"))
    rep.add("SMOKE", smoke)

    # ---------------- baseline capture + determinism twin ----------------
    base_png = str(runs / "baseline.png")
    twin_png = str(runs / "baseline_twin.png")
    ok, code, out, err, dt = screenshot(binary, base_args, [], base_png, k, runlog)
    bl = [f"baseline: {'ok' if ok else 'FAILED'} in {dt:.1f}s"]
    m = RE_CHUNKS.search(out)
    if m:
        bl.append(f"world settle: {m.group(1)} chunks in {m.group(2)}s")
    if Path(base_png).exists():
        bl.append(f"png bytes: {Path(base_png).stat().st_size}")
    if not ok:
        rep.fails.append(("capture", "baseline", f"exit {code}: {err.splitlines()[-1] if err else 'no png'}"))
        rep.add("BASELINE", bl)
        Path(ROOT / args.out).write_text(rep.render())
        runlog.close()
        return 1
    ok2, code, out, err, dt2 = screenshot(binary, base_args, [], twin_png, k, runlog)
    d, code, dout, derr = diff_pair(binary, base_png, twin_png, runlog)
    if d:
        bl.append(f"determinism twin: differing {d['differ']} of {d['total']}  MAE {d['mae']:.4f}  max {d['max']}"
                  + ("  (DETERMINISTIC)" if d["differ"] == 0 else ""))
        if d["differ"] != 0:
            rep.suspects.append(("determinism", "baseline twin",
                                 f"two identical captures differ: MAE {d['mae']}"))
    else:
        bl.append(f"determinism twin: diff failed to run (exit {code})")
        rep.fails.append(("smoke", "twin diff", f"exit {code}"))
    bl.append(f"self-diff sanity + twin capture took {dt2:.1f}s total {dt+dt2:.1f}s")
    rep.add("BASELINE", bl)
    print("baseline captured", flush=True)

    # ---------------- baseline bench captures adapter id ----------------
    ok, code, bres, bout, berr = bench(binary, W, H, k.bench_frames, [], k, runlog)
    bench_lines = []
    base_perf = None
    if ok:
        rep.line("ENVIRONMENT", f"gpu adapter:  {bres['adapter'] or 'unreported'}")
        for b in bres["blocks"]:
            if b.get("hiz") == "off":
                base_perf = b
        if base_perf is None and bres["blocks"]:
            base_perf = bres["blocks"][0]
        if base_perf:
            bench_lines.append(
                f"baseline bench: wall {base_perf.get('wall_ms', 0):.2f}ms "
                f"({base_perf.get('fps', 0)} fps) p99 {base_perf.get('p99_ms', 0):.2f} "
                f"gpu total {base_perf.get('gpu_ms', 0):.2f}ms "
                f"({base_perf.get('chunks', '?')} chunks, {k.bench_frames} frames)")
        for b in bres["blocks"]:
            bench_lines.append(
                f"  hi-z {b.get('hiz'):>3}: wall {b.get('wall_ms', 0):.2f}ms "
                f"gpu {b.get('gpu_ms', 0):.2f}ms chunks {b.get('chunks')}")
        if bres["pairs"]:
            bench_lines.append(f"  pairs marched {bres['pairs'][0]}  deferred {bres['pairs'][1]}")
    else:
        bench_lines.append(f"baseline bench FAILED (exit {code})")
        rep.fails.append(("bench", "baseline", f"exit {code}"))
    rep.add("BENCH BASELINE", bench_lines)
    print("baseline benched", flush=True)

    # ---------------- flag sweep ----------------
    sweep = ["case                       expect  result  differ      MAE    max  seconds  bbox",
             "(rows tagged [ctx] are diffed against their paired context baseline, not the default capture)"]
    ctx_cache = {}
    n = 0
    total = len(CASES)
    t_sweep = time.time()
    for cid, extra, expect in CASES:
        if args.cases and args.cases not in cid:
            continue
        n += 1
        png = str(runs / f"flag_{cid}.png")
        ok, code, out, err, dtc = screenshot(binary, base_args, extra, png, k, runlog)
        if not ok:
            tail = (err.splitlines() or out.splitlines() or ["<no output>"])[-1]
            sweep.append(f"{cid:26s} {expect:6s} FAIL    {'-':>6s} {'-':>7s} {'-':>4s} {dtc:7.1f}  exit {code}")
            rep.fails.append(("flag", cid, f"exit {code}: {tail[:160]}"))
            print(f"[{n}/{total}] {cid}: FAIL (exit {code})", flush=True)
            continue
        ref, ref_tag = base_png, ""
        if cid in CTX:
            key = tuple(CTX[cid])
            ref = ctx_cache.get(key)
            if ref is None:
                ref = str(runs / f"ctx_{len(ctx_cache)}_{cid}.png")
                okc, codec, oc, ec, _ = screenshot(binary, base_args, list(key), ref, k, runlog)
                if not okc:
                    tail = (ec.splitlines() or oc.splitlines() or ["<no output>"])[-1]
                    sweep.append(f"{cid:26s} {expect:6s} FAIL    {'-':>6s} {'-':>7s} {'-':>4s} {dtc:7.1f}  ctx exit {codec}")
                    rep.fails.append(("flag", cid, f"context baseline exit {codec}: {tail[:140]}"))
                    continue
                ctx_cache[key] = ref
            ref_tag = " [ctx]"
        d, code, dout, derr = diff_pair(binary, ref, png, runlog)
        if d is None:
            sweep.append(f"{cid:26s} {expect:6s} FAIL    {'-':>6s} {'-':>7s} {'-':>4s} {dtc:7.1f}  diff exit {code}")
            rep.fails.append(("flag", cid, f"diff exit {code}"))
            continue
        verdict = "ok"
        if expect == UN and d["differ"] != 0:
            verdict = "FAIL"
            rep.fails.append(("flag", cid,
                              f"restated defaults but {d['differ']} px differ (MAE {d['mae']:.4f})"))
        elif expect == CH and d["differ"] == 0:
            verdict = "SUSPECT"
            rep.suspects.append(("flag", cid, "arm moved zero pixels vs baseline" + (" (vs paired context)" if ref_tag else "")))
        elif expect == INERT and d["differ"] == 0:
            verdict = "inert-ok"
            rep.inert.append(("flag", cid, INERT_WHY.get(cid, "no trigger in this scene")))
        elif expect == INERT and d["differ"] != 0:
            verdict = "ok"
            rep.notes.append(f"flag {cid}: expected inert but moved {d['differ']} px -- the trigger exists after all; keep an eye on repeat runs")
        frac = 100.0 * d["differ"] / max(d["total"], 1)
        sweep.append(f"{cid:26s} {expect:6s} {verdict:6s} {d['differ']:6d} {d['mae']:7.4f} {d['max']:4d} "
                     f"{dtc:7.1f}  {frac:5.1f}% of frame; bbox {d['bbox'][:44]}{ref_tag}")
        print(f"[{n}/{total}] {cid}: {verdict} (differ {d['differ']})", flush=True)
    sweep.append(f"sweep wall time: {time.time() - t_sweep:.0f}s for {n} cases")
    rep.add("FLAG SWEEP (baseline vs variant)", sweep)

    # ---------------- perf sweep ----------------
    if not args.skip_perf and base_perf:
        plines = [f"baseline: wall {base_perf.get('wall_ms', 0):.2f}ms "
                  f"gpu {base_perf.get('gpu_ms', 0):.2f}ms (hi-z off block)",
                  "case                  wall_ms    fps  p99_ms   gpu_ms  d_wall%  d_gpu%"]
        for cid, extra in PERF_CASES:
            ok, code, bres, bout, berr = bench(binary, W, H, k.bench_frames, extra, k, runlog)
            if not ok:
                plines.append(f"{cid:20s}  FAILED exit {code}")
                rep.fails.append(("perf", cid, f"bench exit {code}"))
                continue
            blk = next((b for b in bres["blocks"] if b.get("hiz") == "off"), bres["blocks"][0])
            dw = 100.0 * (blk.get("wall_ms", 0) / max(base_perf.get("wall_ms", 1e-9), 1e-9) - 1.0)
            dg = 100.0 * (blk.get("gpu_ms", 0) / max(base_perf.get("gpu_ms", 1e-9), 1e-9) - 1.0)
            plines.append(f"{cid:20s} {blk.get('wall_ms', 0):8.2f} {blk.get('fps', 0):6d} "
                          f"{blk.get('p99_ms', 0):8.2f} {blk.get('gpu_ms', 0):7.2f} "
                          f"{dw:8.1f} {dg:8.1f}")
            if dw > 30:
                rep.notes.append(f"perf: {cid} wall +{dw:.0f}% over baseline")
            print(f"perf {cid}: wall {blk.get('wall_ms', 0):.2f}ms ({dw:+.0f}%)", flush=True)
        rep.add("PERF SWEEP (bench-frames, hi-z off block)", plines)

        # Thermal/order drift: re-run the untouched baseline bench once more at
        # the end. A machine that heats up across the sweep makes late cases
        # look slower; this number calibrates how much trust d_wall% deserves.
        ok, code, bres, bout, berr = bench(binary, W, H, k.bench_frames, [], k, runlog)
        if ok:
            blk = next((b for b in bres["blocks"] if b.get("hiz") == "off"), bres["blocks"][0])
            drift = 100.0 * (blk.get("wall_ms", 0) / max(base_perf.get("wall_ms", 1e-9), 1e-9) - 1.0)
            rep.line("PERF SWEEP (bench-frames, hi-z off block)",
                     f"drift: baseline re-run at end {blk.get('wall_ms', 0):.2f}ms "
                     f"({drift:+.1f}% vs first) -- treat deltas within about ±{max(5.0, abs(drift)):.0f}% as order noise")
            if drift > 15:
                rep.notes.append(f"perf: baseline drifted +{drift:.0f}% across the sweep (thermals?) -- deltas smaller than that are not trustworthy")

    # ---------------- functional: journals, terrain, lookbook, cargo test --
    fj = runs / "journal.journal"
    okS, code, out, err, dtS = screenshot(binary, base_args,
                                          ["--save-edits", str(fj)],
                                          str(runs / "journal_a.png"), k, runlog)
    okL, code2, out2, err2, dtL = screenshot(binary, base_args,
                                             ["--load-edits", str(fj)],
                                             str(runs / "journal_b.png"), k, runlog)
    if okS and okL:
        d, code3, dout, derr = diff_pair(binary, str(runs / "journal_a.png"),
                                         str(runs / "journal_b.png"), runlog)
        if d is not None:
            v = "ok" if d["differ"] == 0 else "FAIL"
            rep.add("FUNCTIONAL", [
                f"edit-journal replay: save-capture vs load-capture differing {d['differ']} "
                f"(MAE {d['mae']:.4f})  [{v}]",
            ])
            if d["differ"] != 0:
                rep.fails.append(("functional", "journal replay",
                                  f"{d['differ']} px differ after replay"))
        else:
            rep.add("FUNCTIONAL", [f"edit-journal replay: diff failed exit {code3}"])
    else:
        rep.add("FUNCTIONAL", [
            f"edit-journal replay: capture failed (save {okS} exit {code}, load {okL} exit {code2})"])
        rep.fails.append(("functional", "journal replay", "capture failed"))

    code, out, err, dtt = run([binary, "--bench-terrain", str(k.terrain_chunks)],
                              k.bench_timeout, runlog)
    chunk_us = re.search(r"per chunk \(us\)\s*\n((?:\s+.*\n)+)", out)
    if code == 0:
        tail = [l.strip() for l in out.splitlines() if l.strip()][:1]
        per = [l.strip() for l in out.splitlines() if "worldgen" in l or "tree build" in l
               or "lighting" in l or "chunks:" in l]
        rep.line("FUNCTIONAL", f"--bench-terrain {k.terrain_chunks}: exit 0 in {dtt:.1f}s | "
                 + " | ".join(per[:4]))
    else:
        rep.line("FUNCTIONAL", f"--bench-terrain: exit {code}")
        rep.fails.append(("functional", "bench-terrain", f"exit {code}"))

    if not args.skip_lookbook:
        lb = runs / "lookbook"
        lb.mkdir(exist_ok=True)
        code, out, err, dtl = run([binary, "--lookbook", str(lb), *base_args],
                                  k.lookbook_timeout, runlog)
        png = lb / "lookbook.png"
        if code == 0 and png.exists():
            rep.line("FUNCTIONAL", f"--lookbook: ok in {dtl:.1f}s ({png.stat().st_size} bytes)")
        else:
            rep.line("FUNCTIONAL", f"--lookbook: exit {code}, png {'present' if png.exists() else 'missing'}")
            rep.fails.append(("functional", "lookbook", f"exit {code}"))

    if not args.skip_tests:
        code, out, err, dtc = run(["cargo", "test", "--release"], k.cargo_test_timeout, runlog)
        passed = failed = 0
        suites = 0
        failing = []
        for line in (out + err).splitlines():
            m = RE_TESTLINE.search(line)
            if m:
                suites += 1
                passed += int(m.group(2))
                failed += int(m.group(3))
                if m.group(1) != "ok":
                    failing.append(line.strip())
        v = "ok" if code == 0 and failed == 0 else "FAIL"
        rep.line("FUNCTIONAL", f"cargo test --release: {passed} passed, {failed} failed "
                 f"across {suites} suites in {dtc:.0f}s  [{v}]")
        if failed or code != 0:
            rep.fails.append(("functional", "cargo test", f"{failed} failed, exit {code}"))
            rep.line("FUNCTIONAL", "failing suites:\n  " + "\n  ".join(failing[:10]))
            rep.line("FUNCTIONAL", "test stderr tail:\n" + "\n".join((out + err).splitlines()[-30:]))

    # ---------------- write ----------------
    text = rep.render()
    Path(ROOT / args.out).write_text(text)
    runlog.close()
    n_fail = len(rep.fails)
    n_sus = len(rep.suspects)
    print(f"\nreport: {args.out}  (log: {args.out}.log, runs: {args.runs_dir}/)")
    print(f"FAIL {n_fail}, SUSPECT {n_sus}")
    return 0 if n_fail == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
