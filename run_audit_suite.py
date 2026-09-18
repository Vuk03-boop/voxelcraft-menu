#!/usr/bin/env python3
"""
VoxelCraft Performance & Regression Audit Suite
================================================
Comprehensive test suite for evaluating performance deltas, rendering differences,
and subsystem invariants after each flag is applied against baseline.

Outputs a consolidated, human- and machine-readable report to: uploadme.txt

Usage:
    python3 run_audit_suite.py                # Standard full audit
    python3 run_audit_suite.py --quick        # Quick check with key flags & fewer frames
    python3 run_audit_suite.py --frames 120   # High-precision benchmark
    python3 run_audit_suite.py --help         # Show all options
"""

import argparse
import datetime
import hashlib
import json
import os
import platform
import re
import shutil
import struct
import subprocess
import sys
import time
import zlib
from pathlib import Path

# ==============================================================================
# FLAG CATALOG & CLASSIFICATION
# ==============================================================================

FLAG_CATALOG = [
    # --- Water & Fluids ---
    {
        "flag": "--no-water",
        "category": "Water & Fluids",
        "desc": "Completely remove water (sea_level=0)",
        "expected_diff": "major",
        "quick": True,
    },
    {
        "flag": "--no-water-reflect",
        "category": "Water & Fluids",
        "desc": "Disable traced reflection rays on water",
        "expected_diff": "moderate",
        "quick": True,
    },
    {
        "flag": "--no-water-refract",
        "category": "Water & Fluids",
        "desc": "Disable traced refraction rays through water",
        "expected_diff": "moderate",
        "quick": False,
    },
    {
        "flag": "--no-waves",
        "category": "Water & Fluids",
        "desc": "Flatten water wave micro-normals (wave_amp=0)",
        "expected_diff": "moderate",
        "quick": True,
    },
    {
        "flag": "--no-wave-aniso",
        "category": "Water & Fluids",
        "desc": "Disable anisotropic wave filtering",
        "expected_diff": "subtle",
        "quick": False,
    },
    {
        "flag": "--no-wave-shoal",
        "category": "Water & Fluids",
        "desc": "Disable shallow water wave damping",
        "expected_diff": "subtle",
        "quick": False,
    },
    {
        "flag": "--no-wave-fill",
        "category": "Water & Fluids",
        "desc": "Halve wave octaves from 8 to 4",
        "expected_diff": "subtle",
        "quick": False,
    },
    {
        "flag": "--no-shore-wet",
        "category": "Water & Fluids",
        "desc": "Dry sand at waterline (disable wet darkening)",
        "expected_diff": "subtle",
        "quick": False,
    },
    {
        "flag": "--no-shore-foam",
        "category": "Water & Fluids",
        "desc": "Disable procedural shoreline foam",
        "expected_diff": "subtle",
        "quick": False,
    },
    {
        "flag": "--no-water-sec",
        "category": "Water & Fluids",
        "desc": "Disable half-res secondary traced water rays",
        "expected_diff": "subtle",
        "quick": False,
    },
    {
        "flag": "--water-look",
        "category": "Water & Fluids",
        "desc": "Enable turquoise shoreline ripple appearance",
        "expected_diff": "moderate",
        "quick": True,
    },
    {
        "flag": "--caustics",
        "category": "Water & Fluids",
        "desc": "Enable shallow underwater animated sun caustics",
        "expected_diff": "moderate",
        "quick": False,
    },

    # --- Shadows & Lighting ---
    {
        "flag": "--no-shadows",
        "category": "Shadows & Lighting",
        "desc": "Disable all sun shadows completely",
        "expected_diff": "major",
        "quick": True,
    },
    {
        "flag": "--soft-shadows",
        "category": "Shadows & Lighting",
        "desc": "Enable soft shadow penumbrae",
        "expected_diff": "moderate",
        "quick": True,
    },
    {
        "flag": "--soft-shadow-hq",
        "category": "Shadows & Lighting",
        "desc": "High-quality soft shadow filtering",
        "expected_diff": "subtle",
        "quick": False,
    },
    {
        "flag": "--no-probe-shadow",
        "category": "Shadows & Lighting",
        "desc": "Disable diffuse bounce shadow checking",
        "expected_diff": "moderate",
        "quick": False,
    },
    {
        "flag": "--no-distant-shadows",
        "category": "Shadows & Lighting",
        "desc": "Disable shadows on distant LODs",
        "expected_diff": "subtle",
        "quick": False,
    },
    {
        "flag": "--no-ao",
        "category": "Shadows & Lighting",
        "desc": "Disable ambient occlusion",
        "expected_diff": "moderate",
        "quick": True,
    },
    {
        "flag": "--no-light-rgb",
        "category": "Shadows & Lighting",
        "desc": "Disable packed RGB block light flood",
        "expected_diff": "moderate",
        "quick": False,
    },
    {
        "flag": "--legacy-lighting",
        "category": "Shadows & Lighting",
        "desc": "Revert to legacy pre-repair lighting model",
        "expected_diff": "moderate",
        "quick": False,
    },
    {
        "flag": "--compact-shade-hit",
        "category": "Shadows & Lighting",
        "desc": "Compact shading function A/B experiment",
        "expected_diff": "none",
        "quick": True,
    },
    {
        "flag": "--shadow-pass",
        "category": "Shadows & Lighting",
        "desc": "Dedicated primary shadow pass",
        "expected_diff": "none",
        "quick": False,
    },

    # --- Biomes, Terrain & Foliage ---
    {
        "flag": "--no-biomes",
        "category": "Biomes & Terrain",
        "desc": "Revert worldgen to single plains biome",
        "expected_diff": "major",
        "quick": True,
    },
    {
        "flag": "--no-foliage",
        "category": "Biomes & Terrain",
        "desc": "Remove grass and wildflower cross-quad tufts",
        "expected_diff": "moderate",
        "quick": True,
    },
    {
        "flag": "--no-meadow",
        "category": "Biomes & Terrain",
        "desc": "Remove coarse meadow recoloring",
        "expected_diff": "subtle",
        "quick": False,
    },
    {
        "flag": "--no-tint",
        "category": "Biomes & Terrain",
        "desc": "Disable vegetation biome tinting",
        "expected_diff": "moderate",
        "quick": False,
    },
    {
        "flag": "--no-leaf-cutout",
        "category": "Biomes & Terrain",
        "desc": "Disable 4³ sub-voxel leaf canopy masks",
        "expected_diff": "moderate",
        "quick": False,
    },
    {
        "flag": "--foliage-rich",
        "category": "Biomes & Terrain",
        "desc": "Enable varied species vegetation hash",
        "expected_diff": "subtle",
        "quick": False,
    },
    {
        "flag": "--canopy-relief",
        "category": "Biomes & Terrain",
        "desc": "Enable canopy directional shading relief",
        "expected_diff": "subtle",
        "quick": False,
    },
    {
        "flag": "--tree-blue-noise",
        "category": "Biomes & Terrain",
        "desc": "Blue-noise coarse canopy thinning",
        "expected_diff": "subtle",
        "quick": False,
    },
    {
        "flag": "--grass-dense",
        "category": "Biomes & Terrain",
        "desc": "Dense two-height tufts at LOD 0",
        "expected_diff": "moderate",
        "quick": False,
    },
    {
        "flag": "--wind-sway",
        "category": "Biomes & Terrain",
        "desc": "Animated foliage wind swaying",
        "expected_diff": "subtle",
        "quick": False,
    },

    # --- Atmosphere & Sky ---
    {
        "flag": "--cloud-cover 0",
        "category": "Atmosphere & Sky",
        "desc": "Disable high-altitude cloud deck entirely",
        "expected_diff": "major",
        "quick": True,
    },
    {
        "flag": "--sky-cool",
        "category": "Atmosphere & Sky",
        "desc": "Switch to cool midday sky gradient",
        "expected_diff": "moderate",
        "quick": False,
    },
    {
        "flag": "--grade cine",
        "category": "Atmosphere & Sky",
        "desc": "Filmic S-curve tone curve grading",
        "expected_diff": "moderate",
        "quick": False,
    },
    {
        "flag": "--grade warm",
        "category": "Atmosphere & Sky",
        "desc": "Warm sunset presentation color grading",
        "expected_diff": "moderate",
        "quick": False,
    },
    {
        "flag": "--no-sky-tint",
        "category": "Atmosphere & Sky",
        "desc": "Disable sky atmospheric tinting",
        "expected_diff": "moderate",
        "quick": False,
    },
    {
        "flag": "--no-sky-specular",
        "category": "Atmosphere & Sky",
        "desc": "Disable specular sky reflection term",
        "expected_diff": "subtle",
        "quick": False,
    },
    {
        "flag": "--no-terrain-shafts",
        "category": "Atmosphere & Sky",
        "desc": "Disable terrain crepuscular godrays",
        "expected_diff": "subtle",
        "quick": False,
    },

    # --- Pipeline & Acceleration ---
    {
        "flag": "--no-hiz",
        "category": "Pipeline & Acceleration",
        "desc": "Disable Hi-Z occlusion culling (lossless culling test)",
        "expected_diff": "none",
        "quick": True,
    },
    {
        "flag": "--no-taa",
        "category": "Pipeline & Acceleration",
        "desc": "Disable temporal anti-aliasing resolve",
        "expected_diff": "moderate",
        "quick": True,
    },
    {
        "flag": "--no-fade",
        "category": "Pipeline & Acceleration",
        "desc": "Disable LOD cross-fade transitions",
        "expected_diff": "subtle",
        "quick": False,
    },
    {
        "flag": "--isolate-glass",
        "category": "Pipeline & Acceleration",
        "desc": "Dedicated glass shading pass",
        "expected_diff": "none",
        "quick": False,
    },
    {
        "flag": "--snell-bend",
        "category": "Pipeline & Acceleration",
        "desc": "Trace refracted ray inside Snell window",
        "expected_diff": "moderate",
        "quick": False,
    },
    {
        "flag": "--glass-reflect",
        "category": "Pipeline & Acceleration",
        "desc": "Traced glass reflection ray march",
        "expected_diff": "moderate",
        "quick": False,
    },

    # --- Combinations ---
    {
        "flag": "--no-hiz --no-taa",
        "category": "Combinations",
        "desc": "Raw raymarcher (no Hi-Z culling, no TAA)",
        "expected_diff": "moderate",
        "quick": True,
    },
    {
        "flag": "--soft-shadows --water-look --foliage-rich --canopy-relief",
        "category": "Combinations",
        "desc": "Visual Enhancements Combined",
        "expected_diff": "major",
        "quick": True,
    },
    {
        "flag": "--no-water --no-shadows --no-biomes --no-foliage --no-ao",
        "category": "Combinations",
        "desc": "Minimalist Performance Floor",
        "expected_diff": "major",
        "quick": True,
    },
]

# ==============================================================================
# PURE PYTHON PNG DECODER & PIXEL DIFFER
# ==============================================================================

def decode_png(file_path):
    """Decodes a PNG file using pure standard library (struct + zlib)."""
    with open(file_path, "rb") as f:
        data = f.read()

    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError(f"Invalid PNG header: {file_path}")

    pos = 8
    width = height = color_type = None
    idat = bytearray()

    while pos < len(data):
        length, chunk_type = struct.unpack(">I4s", data[pos : pos + 8])
        pos += 8
        chunk_data = data[pos : pos + length]
        pos += length + 4  # skip 4-byte CRC

        if chunk_type == b"IHDR":
            width, height, bit_depth, color_type = struct.unpack(">IIBB", chunk_data[:10])
            if bit_depth != 8:
                raise ValueError(f"Unsupported bit depth: {bit_depth}")
        elif chunk_type == b"IDAT":
            idat.extend(chunk_data)
        elif chunk_type == b"IEND":
            break

    raw = zlib.decompress(idat)
    bpp = 4 if color_type == 6 else 3 if color_type == 2 else 1
    stride = width * bpp
    img = bytearray(width * height * bpp)

    r_pos = 0
    prev_line = bytearray(stride)
    curr_line = bytearray(stride)
    out_pos = 0

    for y in range(height):
        filter_type = raw[r_pos]
        r_pos += 1
        scanline = raw[r_pos : r_pos + stride]
        r_pos += stride

        for x in range(stride):
            left = curr_line[x - bpp] if x >= bpp else 0
            up = prev_line[x]
            up_left = prev_line[x - bpp] if x >= bpp else 0

            if filter_type == 0:
                val = scanline[x]
            elif filter_type == 1:
                val = (scanline[x] + left) & 0xFF
            elif filter_type == 2:
                val = (scanline[x] + up) & 0xFF
            elif filter_type == 3:
                val = (scanline[x] + (left + up) // 2) & 0xFF
            elif filter_type == 4:
                p = left + up - up_left
                pa, pb, pc = abs(p - left), abs(p - up), abs(p - up_left)
                pr = left if pa <= pb and pa <= pc else (up if pb <= pc else up_left)
                val = (scanline[x] + pr) & 0xFF
            else:
                val = scanline[x]

            curr_line[x] = val

        img[out_pos : out_pos + stride] = curr_line
        prev_line[:] = curr_line
        out_pos += stride

    return width, height, bpp, img


def compare_images(img1_path, img2_path):
    """Calculates differing pixels, MAE, max delta, and MSE between two images."""
    try:
        w1, h1, bpp1, raw1 = decode_png(img1_path)
        w2, h2, bpp2, raw2 = decode_png(img2_path)
    except Exception as e:
        return {"error": str(e)}

    if (w1, h1) != (w2, h2):
        return {"error": f"Resolution mismatch: {w1}x{h1} vs {w2}x{h2}"}

    total_pixels = w1 * h1
    channels = 3 if bpp1 in (3, 4) else 1
    step1, step2 = bpp1, bpp2

    diff_pixels = 0
    total_abs_diff = 0
    total_sq_diff = 0
    max_delta = 0

    for i in range(total_pixels):
        off1 = i * step1
        off2 = i * step2
        has_diff = False

        for c in range(channels):
            d = abs(raw1[off1 + c] - raw2[off2 + c])
            if d > 0:
                has_diff = True
                total_abs_diff += d
                total_sq_diff += d * d
                if d > max_delta:
                    max_delta = d

        if has_diff:
            diff_pixels += 1

    sample_count = total_pixels * channels
    mae = total_abs_diff / max(1, sample_count)
    mse = total_sq_diff / max(1, sample_count)
    diff_pct = (diff_pixels / max(1, total_pixels)) * 100.0

    # Categorize visual impact
    if diff_pixels == 0:
        category = "Bit-Exact (0 diff)"
    elif diff_pct < 0.05 and mae < 0.2:
        category = "Perceptually Identical"
    elif mae < 2.0:
        category = "Subtle / Minor"
    elif mae < 10.0:
        category = "Noticeable / Shading"
    else:
        category = "Major Visual Shift"

    return {
        "total_pixels": total_pixels,
        "diff_pixels": diff_pixels,
        "diff_pct": diff_pct,
        "mae": mae,
        "mse": mse,
        "max_delta": max_delta,
        "category": category,
    }

# ==============================================================================
# BENCHMARK OUTPUT PARSER
# ==============================================================================

def parse_benchmark_output(stdout_text):
    """Extracts GPU name, wall times, and per-pass breakdowns from --bench-frames."""
    res = {
        "gpu": "Unknown GPU",
        "hiz_on": None,
        "hiz_off": None,
    }

    gpu_match = re.search(r"gpu:\s*(.+)", stdout_text)
    if gpu_match:
        res["gpu"] = gpu_match.group(1).strip()

    hiz_blocks = re.findall(
        r"---\s*(\d+x\d+)\s+hi-z\s+(\w+)\s+taa\s+(\w+)\s+(\d+)\s+chunks\s*---\s*([\s\S]*?)(?=(?:---\s*\d+x\d+|\Z))",
        stdout_text,
    )

    for res_str, hiz_state, taa_state, chunks, body in hiz_blocks:
        key = "hiz_on" if hiz_state == "on" else "hiz_off"
        wall_match = re.search(r"wall\s+median\s+([\d\.]+)ms\s+\(([\d\.]+)\s+fps\)\s+p99\s+([\d\.]+)ms", body)
        gpu_tot_match = re.search(r"gpu\s+total\s+([\d\.]+)ms", body)
        gpu_passes = re.search(
            r"gpu\s+select\s+([\d\.]+)\s+march\s+([\d\.]+)\s+hiz\s+([\d\.]+)\s+recover\s+([\d\.]+)\s+march2\s+([\d\.]+)\s+resolve\s+([\d\.]+)\s+taa\s+([\d\.]+)\s+shaft\s+([\d\.]+)",
            body,
        )

        info = {
            "res": res_str,
            "chunks": int(chunks),
            "wall_median_ms": float(wall_match.group(1)) if wall_match else None,
            "fps": float(wall_match.group(2)) if wall_match else None,
            "p99_ms": float(wall_match.group(3)) if wall_match else None,
            "gpu_total_ms": float(gpu_tot_match.group(1)) if gpu_tot_match else None,
            "passes": None,
        }

        if gpu_passes:
            info["passes"] = {
                "select": float(gpu_passes.group(1)),
                "march": float(gpu_passes.group(2)),
                "hiz": float(gpu_passes.group(3)),
                "recover": float(gpu_passes.group(4)),
                "march2": float(gpu_passes.group(5)),
                "resolve": float(gpu_passes.group(6)),
                "taa": float(gpu_passes.group(7)),
                "shaft": float(gpu_passes.group(8)),
            }

        res[key] = info

    return res


def parse_terrain_output(stdout_text):
    """Extracts worldgen benchmark metrics."""
    res = {}
    solid_match = re.search(r"solid voxels:\s*(\d+)", stdout_text)
    total_us = re.search(r"total\s+([\d\.]+)", stdout_text)
    geom_bytes = re.search(r"geometry\s+[\d\.]+\s+[A-Z]+\s+\(([\d\.]+)\s+bytes/voxel\)", stdout_text)
    attr_bytes = re.search(r"attributes\+light\s+[\d\.]+\s+[A-Z]+\s+\(([\d\.]+)\s+bytes/voxel\)", stdout_text)

    if solid_match:
        res["solid_voxels"] = int(solid_match.group(1))
    if total_us:
        res["us_per_chunk"] = float(total_us.group(1))
    if geom_bytes:
        res["geom_bytes_per_voxel"] = float(geom_bytes.group(1))
    if attr_bytes:
        res["attr_bytes_per_voxel"] = float(attr_bytes.group(1))
    return res

# ==============================================================================
# AUDIT RUNNER CLASS
# ==============================================================================

class VoxelCraftAuditor:
    def __init__(self, args):
        self.args = args
        self.root_dir = Path.cwd()
        self.artifacts_dir = self.root_dir / "audit_artifacts"
        self.output_file = self.root_dir / args.output
        self.target_bin = self._resolve_binary(args.bin)
        self.results = {
            "meta": {
                "timestamp": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                "os": platform.platform(),
                "arch": platform.machine(),
                "python": platform.python_version(),
            },
            "system_tests": {},
            "baseline": {},
            "flag_results": [],
            "anomalies": [],
        }

    def _resolve_binary(self, custom_bin):
        if custom_bin:
            p = Path(custom_bin).resolve()
            if p.exists() and os.access(p, os.X_OK):
                return [str(p)]
            return custom_bin.split()

        # Check compiled release binary
        cand_unix = self.root_dir / "target" / "release" / "voxelcraft"
        cand_win = self.root_dir / "target" / "release" / "voxelcraft.exe"
        if cand_unix.exists() and os.access(cand_unix, os.X_OK):
            return [str(cand_unix)]
        if cand_win.exists():
            return [str(cand_win)]

        # Check if cargo is in PATH
        cargo = shutil.which("cargo")
        if cargo:
            return [cargo, "run", "--release", "--"]

        return None

    def _run_cmd(self, cmd_args, timeout=120):
        """Executes a command and returns exit code, stdout, and stderr."""
        try:
            p = subprocess.run(
                cmd_args,
                cwd=str(self.root_dir),
                capture_output=True,
                text=True,
                timeout=timeout,
            )
            return p.returncode, p.stdout, p.stderr
        except subprocess.TimeoutExpired:
            return -1, "", f"Command timed out after {timeout} seconds"
        except Exception as e:
            return -1, "", str(e)

    def prepare(self):
        print("=" * 80)
        print(" VOXELCRAFT PERFORMANCE & REGRESSION AUDIT SUITE")
        print("=" * 80)
        print(f"Timestamp: {self.results['meta']['timestamp']}")
        print(f"Host OS:   {self.results['meta']['os']} ({self.results['meta']['arch']})")
        print(f"Directory: {self.root_dir}")

        if not self.target_bin:
            print("\n[!] FATAL: Rust/Cargo or compiled binary not found in PATH!")
            print("To run this audit on your machine:")
            print("  1. Make sure Rust & Cargo are installed (https://rustup.rs)")
            print("  2. Run: cargo build --release")
            print("  3. Re-run: python3 run_audit_suite.py")
            sys.exit(1)

        print(f"Runner:    {' '.join(self.target_bin)}")
        self.artifacts_dir.mkdir(exist_ok=True)

    def run_system_verification(self):
        """Runs cargo test and terrain benchmarks."""
        print("\n[1/4] Running System Verification & Core Invariant Tests...")

        # 1. Cargo Tests
        if not self.args.skip_cargo_test and shutil.which("cargo"):
            print("  * Running: cargo test --release ... ", end="", flush=True)
            code, out, err = self._run_cmd(["cargo", "test", "--release"], timeout=180)
            if code == 0:
                passed_match = re.search(r"test result: ok\.\s*(\d+)\s+passed;\s*(\d+)\s+failed;\s*(\d+)\s+ignored", out)
                if passed_match:
                    passed, failed, ignored = passed_match.groups()
                    print(f"OK ({passed} passed, {failed} failed, {ignored} ignored)")
                    self.results["system_tests"]["cargo_test"] = {
                        "status": "PASS",
                        "passed": int(passed),
                        "failed": int(failed),
                        "ignored": int(ignored),
                    }
                else:
                    print("OK")
                    self.results["system_tests"]["cargo_test"] = {"status": "PASS"}
            else:
                print("FAIL")
                self.results["system_tests"]["cargo_test"] = {
                    "status": "FAIL",
                    "error": err[:400] if err else out[:400],
                }
                self.results["anomalies"].append("Cargo test suite failed.")
        else:
            print("  * Skipping cargo test (omitted or cargo unavailable)")
            self.results["system_tests"]["cargo_test"] = {"status": "SKIPPED"}

        # 2. Terrain Generation Benchmark
        print("  * Running terrain generation bench (--bench-terrain 32) ... ", end="", flush=True)
        t_cmd = list(self.target_bin) + ["--bench-terrain", "32"]
        code, out, err = self._run_cmd(t_cmd, timeout=60)
        if code == 0:
            t_metrics = parse_terrain_output(out)
            us_chunk = t_metrics.get("us_per_chunk", "N/A")
            geom_b = t_metrics.get("geom_bytes_per_voxel", "N/A")
            print(f"OK ({us_chunk} us/chunk, {geom_b} bytes/voxel)")
            self.results["system_tests"]["terrain_bench"] = {
                "status": "PASS",
                "metrics": t_metrics,
            }
        else:
            print("FAIL")
            self.results["system_tests"]["terrain_bench"] = {"status": "FAIL", "error": err}
            self.results["anomalies"].append("Terrain benchmark failed.")

    def run_baseline(self):
        """Runs the unflagged baseline benchmark and captures reference screenshot."""
        print("\n[2/4] Capturing Baseline Reference Performance & Render...")
        base_png = self.artifacts_dir / "baseline.png"

        # 1. Baseline Screenshot
        print(f"  * Capturing baseline frame ({self.args.width}x{self.args.height}) ... ", end="", flush=True)
        shot_cmd = list(self.target_bin) + [
            "--screenshot",
            str(base_png),
            "--width",
            str(self.args.width),
            "--height",
            str(self.args.height),
        ]
        code, out, err = self._run_cmd(shot_cmd, timeout=60)
        if code != 0 or not base_png.exists():
            print("FAIL")
            print(f"[!] Error: Failed to capture baseline screenshot: {err or out}")
            sys.exit(1)

        hasher = hashlib.sha256()
        with open(base_png, "rb") as f:
            hasher.update(f.read())
        base_hash = hasher.hexdigest()[:12]
        print(f"OK (SHA256: {base_hash})")

        # 2. Baseline Benchmark
        print(f"  * Benchmarking baseline ({self.args.frames} frames) ... ", end="", flush=True)
        bench_cmd = list(self.target_bin) + [
            "--bench-frames",
            str(self.args.frames),
            "--width",
            str(self.args.width),
            "--height",
            str(self.args.height),
        ]
        code, out, err = self._run_cmd(bench_cmd, timeout=90)
        if code != 0:
            print("FAIL")
            print(f"[!] Error: Baseline benchmark failed: {err or out}")
            sys.exit(1)

        bench_data = parse_benchmark_output(out)
        hiz_on = bench_data.get("hiz_on", {})
        fps = hiz_on.get("fps", 0.0)
        wall_ms = hiz_on.get("wall_median_ms", 0.0)
        gpu_name = bench_data.get("gpu", "Unknown GPU")
        print(f"OK ({fps:.1f} FPS, {wall_ms:.2f} ms | GPU: {gpu_name})")

        self.results["baseline"] = {
            "gpu": gpu_name,
            "hash": base_hash,
            "image_path": str(base_png),
            "bench": bench_data,
        }

    def run_flag_matrix(self):
        """Sweeps flags against baseline, comparing performance and rendering differences."""
        print("\n[3/4] Sweeping Feature Flags Against Baseline...")
        flags_to_test = [
            f for f in FLAG_CATALOG
            if (not self.args.quick or f["quick"]) and (not self.args.category or f["category"].lower() == self.args.category.lower())
        ]

        if self.args.flags:
            custom_list = [f.strip() for f in self.args.flags.split(",") if f.strip()]
            flags_to_test = [{"flag": c, "category": "Custom", "desc": "User requested flag", "quick": True} for c in custom_list]

        base_png = self.results["baseline"]["image_path"]
        base_hiz = self.results["baseline"]["bench"].get("hiz_on") or {}
        base_ms = base_hiz.get("wall_median_ms") or 1.0
        base_fps = base_hiz.get("fps") or 1.0

        total = len(flags_to_test)
        for idx, item in enumerate(flags_to_test, 1):
            flag_str = item["flag"]
            flag_tokens = flag_str.split()
            slug = re.sub(r"[^\w-]", "_", flag_str.replace("--", ""))
            flag_png = self.artifacts_dir / f"{slug}.png"

            print(f"  [{idx:2d}/{total:2d}] {flag_str:<32} ... ", end="", flush=True)

            # 1. Screenshot with flag
            shot_cmd = list(self.target_bin) + [
                "--screenshot",
                str(flag_png),
                "--width",
                str(self.args.width),
                "--height",
                str(self.args.height),
            ] + flag_tokens

            code_shot, out_shot, err_shot = self._run_cmd(shot_cmd, timeout=60)

            # 2. Benchmark with flag
            bench_cmd = list(self.target_bin) + [
                "--bench-frames",
                str(self.args.frames),
                "--width",
                str(self.args.width),
                "--height",
                str(self.args.height),
            ] + flag_tokens

            code_bench, out_bench, err_bench = self._run_cmd(bench_cmd, timeout=90)

            if code_shot != 0 or code_bench != 0:
                print("FAILED / CRASH")
                err_msg = (err_shot or out_shot or err_bench or out_bench).strip()[:150]
                self.results["flag_results"].append({
                    "flag": flag_str,
                    "category": item["category"],
                    "desc": item["desc"],
                    "status": "CRASH",
                    "error": err_msg,
                })
                self.results["anomalies"].append(f"Flag '{flag_str}' failed with error: {err_msg}")
                continue

            # Parse benchmark
            bench_data = parse_benchmark_output(out_bench)
            hiz_on = bench_data.get("hiz_on") or {}
            cur_ms = hiz_on.get("wall_median_ms") or 0.0
            cur_fps = hiz_on.get("fps") or 0.0
            cur_p99 = hiz_on.get("p99_ms") or 0.0
            cur_gpu = hiz_on.get("gpu_total_ms") or 0.0

            delta_ms = cur_ms - base_ms
            delta_fps = cur_fps - base_fps
            speedup_pct = ((base_ms - cur_ms) / base_ms) * 100.0

            # Compute image difference
            diff_metrics = compare_images(base_png, flag_png) if flag_png.exists() else {"error": "Image missing"}

            diff_pct_str = f"{diff_metrics.get('diff_pct', 0.0):.2f}%" if "diff_pct" in diff_metrics else "N/A"
            mae_str = f"{diff_metrics.get('mae', 0.0):.2f}" if "mae" in diff_metrics else "N/A"
            cat_str = diff_metrics.get("category", "Error")

            print(f"{cur_fps:5.1f} FPS ({delta_ms:+5.2f}ms) | Diff: {diff_pct_str:>6} (MAE {mae_str:>5}) [{cat_str}]")

            # Check invariant expectations
            health = "HEALTHY"
            if item.get("expected_diff") == "none" and diff_metrics.get("diff_pixels", 0) > 0:
                health = "INVARIANT DRIFT"
                self.results["anomalies"].append(
                    f"Invariant violation: '{flag_str}' is expected to be bit-exact (0 diff), but had {diff_metrics.get('diff_pixels')} differing pixels."
                )

            res_entry = {
                "flag": flag_str,
                "category": item["category"],
                "desc": item["desc"],
                "status": "PASS",
                "health": health,
                "fps": cur_fps,
                "wall_median_ms": cur_ms,
                "p99_ms": cur_p99,
                "gpu_total_ms": cur_gpu,
                "delta_ms": delta_ms,
                "delta_fps": delta_fps,
                "speedup_pct": speedup_pct,
                "passes": hiz_on.get("passes"),
                "diff": diff_metrics,
            }
            self.results["flag_results"].append(res_entry)

    def run_simulation(self):
        """Simulates an audit run for validation and testing of reporting pipeline."""
        print("=" * 80)
        print(" VOXELCRAFT AUDIT SUITE (DRY-RUN SIMULATION MODE)")
        print("=" * 80)
        self.results["system_tests"]["cargo_test"] = {"status": "PASS", "passed": 42, "failed": 0, "ignored": 0}
        self.results["system_tests"]["terrain_bench"] = {
            "status": "PASS",
            "metrics": {"us_per_chunk": 1380.5, "geom_bytes_per_voxel": 0.130, "attr_bytes_per_voxel": 0.450},
        }
        self.results["baseline"] = {
            "gpu": "Simulated NVIDIA GeForce RTX 3050 Laptop GPU",
            "hash": "simulated8a9f",
            "bench": {
                "hiz_on": {
                    "fps": 128.4,
                    "wall_median_ms": 7.79,
                    "p99_ms": 8.85,
                    "gpu_total_ms": 7.15,
                    "chunks": 38,
                    "passes": {
                        "select": 0.14,
                        "march": 1.82,
                        "hiz": 0.12,
                        "recover": 0.08,
                        "march2": 0.25,
                        "resolve": 4.12,
                        "taa": 0.41,
                        "shaft": 0.21,
                    },
                }
            },
        }

        flags_to_sim = [f for f in FLAG_CATALOG if not self.args.quick or f["quick"]]
        for f in flags_to_sim:
            flag_str = f["flag"]
            delta = 0.0
            diff_pct = 0.0
            mae = 0.0
            max_d = 0
            cat = "Bit-Exact (0 diff)"

            if "--no-water" in flag_str:
                delta = -1.15
                diff_pct = 18.4
                mae = 12.3
                max_d = 210
                cat = "Major Visual Shift"
            elif "--no-shadows" in flag_str:
                delta = -1.45
                diff_pct = 24.2
                mae = 18.5
                max_d = 240
                cat = "Major Visual Shift"
            elif "--no-hiz" in flag_str:
                delta = 1.72
                diff_pct = 0.0
                mae = 0.0
                max_d = 0
                cat = "Bit-Exact (0 diff)"
            elif "--no-taa" in flag_str:
                delta = -0.38
                diff_pct = 42.1
                mae = 4.2
                max_d = 120
                cat = "Noticeable / Shading"
            elif "--soft-shadows" in flag_str:
                delta = 0.65
                diff_pct = 5.3
                mae = 2.8
                max_d = 85
                cat = "Noticeable / Shading"
            elif "--no-biomes" in flag_str:
                delta = -0.85
                diff_pct = 31.0
                mae = 14.5
                max_d = 225
                cat = "Major Visual Shift"
            else:
                delta = -0.15
                diff_pct = 2.5
                mae = 1.1
                max_d = 45
                cat = "Subtle / Minor"

            cur_ms = 7.79 + delta
            cur_fps = 1000.0 / cur_ms
            speedup = ((7.79 - cur_ms) / 7.79) * 100.0

            self.results["flag_results"].append({
                "flag": flag_str,
                "category": f["category"],
                "desc": f["desc"],
                "status": "PASS",
                "health": "HEALTHY",
                "fps": cur_fps,
                "wall_median_ms": cur_ms,
                "p99_ms": cur_ms * 1.12,
                "gpu_total_ms": cur_ms * 0.92,
                "delta_ms": delta,
                "delta_fps": cur_fps - 128.4,
                "speedup_pct": speedup,
                "passes": {
                    "select": 0.14,
                    "march": 1.82,
                    "hiz": 0.12,
                    "recover": 0.08,
                    "march2": 0.25,
                    "resolve": 4.12 + delta * 0.7,
                    "taa": 0.41,
                    "shaft": 0.21,
                },
                "diff": {
                    "total_pixels": self.args.width * self.args.height,
                    "diff_pixels": int((self.args.width * self.args.height) * (diff_pct / 100.0)),
                    "diff_pct": diff_pct,
                    "mae": mae,
                    "max_delta": max_d,
                    "category": cat,
                },
            })

        self.write_report()

    def write_report(self):
        """Compiles all metrics into uploadme.txt."""
        print(f"\n[4/4] Writing Comprehensive Audit to {self.output_file.name}...")

        base = self.results["baseline"]
        base_hiz = base["bench"].get("hiz_on") or {}
        base_passes = base_hiz.get("passes") or {}

        lines = []
        lines.append("=" * 110)
        lines.append(" VOXELCRAFT PERFORMANCE & REGRESSION AUDIT REPORT")
        lines.append("=" * 110)
        lines.append(f"Generated:       {self.results['meta']['timestamp']}")
        lines.append(f"Host OS:         {self.results['meta']['os']} ({self.results['meta']['arch']})")
        lines.append(f"Python:          {self.results['meta']['python']}")
        lines.append(f"GPU Adapter:     {base.get('gpu', 'Unknown')}")
        lines.append(f"Resolution:      {self.args.width}x{self.args.height}")
        lines.append(f"Benchmark Iter:  {self.args.frames} frames")
        lines.append("=" * 110)
        lines.append("")

        # Section 1: System Tests
        lines.append("[1/5] SYSTEM HEALTH & SUITE VERIFICATION")
        lines.append("-" * 110)
        c_test = self.results["system_tests"].get("cargo_test", {})
        if c_test.get("status") == "PASS":
            p = c_test.get("passed", "All")
            f = c_test.get("failed", 0)
            i = c_test.get("ignored", 0)
            lines.append(f"  * Cargo Test Suite:      PASSED ({p} passed, {f} failed, {i} ignored)")
        elif c_test.get("status") == "SKIPPED":
            lines.append("  * Cargo Test Suite:      SKIPPED")
        else:
            lines.append(f"  * Cargo Test Suite:      FAILED! (Details: {c_test.get('error')})")

        t_bench = self.results["system_tests"].get("terrain_bench", {})
        if t_bench.get("status") == "PASS":
            m = t_bench.get("metrics", {})
            lines.append(f"  * Terrain Gen Benchmark: PASSED ({m.get('us_per_chunk', 'N/A')} us/chunk | Geom: {m.get('geom_bytes_per_voxel', 'N/A')} B/vox | Attr: {m.get('attr_bytes_per_voxel', 'N/A')} B/vox)")
        else:
            lines.append(f"  * Terrain Gen Benchmark: FAILED! ({t_bench.get('error', 'Unknown')})")
        lines.append("")

        # Section 2: Baseline
        lines.append("[2/5] BASELINE REFERENCE PERFORMANCE")
        lines.append("-" * 110)
        lines.append(f"  * Baseline Wall Median:  {base_hiz.get('wall_median_ms', 0.0):.2f} ms ({base_hiz.get('fps', 0.0):.1f} FPS) | p99: {base_hiz.get('p99_ms', 0.0):.2f} ms")
        lines.append(f"  * Baseline GPU Total:    {base_hiz.get('gpu_total_ms', 0.0):.2f} ms across {base_hiz.get('chunks', 0)} chunks")
        if base_passes:
            lines.append("  * Per-Pass GPU Breakdown (ms):")
            lines.append(
                f"      Select:  {base_passes.get('select', 0.0):.2f} | March: {base_passes.get('march', 0.0):.2f} | Hi-Z: {base_passes.get('hiz', 0.0):.2f} | Recover: {base_passes.get('recover', 0.0):.2f}"
            )
            lines.append(
                f"      March2:  {base_passes.get('march2', 0.0):.2f} | Resolve: {base_passes.get('resolve', 0.0):.2f} | TAA:  {base_passes.get('taa', 0.0):.2f} | Shaft:   {base_passes.get('shaft', 0.0):.2f}"
            )
        lines.append(f"  * Baseline Image SHA256: {base.get('hash')}")
        lines.append("")

        # Section 3: Flag Matrix Table
        lines.append("[3/5] FLAG-BY-FLAG PERFORMANCE & RENDERING DIFFERENCE MATRIX")
        lines.append("-" * 125)
        header = f"{'Flag':<42} {'Status':<6} {'FPS':>6} {'Wall(ms)':>8} {'Delta':>8} {'% Speedup':>10} {'Diff Px(%)':>11} {'MAE':>7} {'MaxD':>5} {'Visual Category':<22}"
        lines.append(header)
        lines.append("-" * 125)

        current_cat = None
        for r in self.results["flag_results"]:
            if r.get("category") != current_cat:
                current_cat = r.get("category")
                lines.append(f"\n--- {current_cat.upper()} ---")

            flag_disp = r['flag']
            if len(flag_disp) > 41:
                flag_disp = flag_disp[:38] + "..."

            if r.get("status") != "PASS":
                lines.append(f"{flag_disp:<42} {'CRASH':<6} {'N/A':>6} {'N/A':>8} {'N/A':>8} {'N/A':>10} {'N/A':>11} {'N/A':>7} {'N/A':>5} {'Error: ' + r.get('error', '')[:20]:<22}")
                continue

            diff = r.get("diff", {})
            diff_pct_str = f"{diff.get('diff_pct', 0.0):.2f}%" if "diff_pct" in diff else "N/A"
            mae_str = f"{diff.get('mae', 0.0):.2f}" if "mae" in diff else "N/A"
            maxd_str = str(diff.get("max_delta", "N/A"))
            cat_str = diff.get("category", "N/A")

            row = (
                f"{flag_disp:<42} "
                f"{r['status']:<6} "
                f"{r['fps']:>6.1f} "
                f"{r['wall_median_ms']:>8.2f} "
                f"{r['delta_ms']:>+8.2f} "
                f"{r['speedup_pct']:>+9.1f}% "
                f"{diff_pct_str:>11} "
                f"{mae_str:>7} "
                f"{maxd_str:>5} "
                f"{cat_str:<22}"
            )
            lines.append(row)

        lines.append("")

        # Section 4: Deep Pass Shifts
        lines.append("[4/5] PER-PASS GPU TIMING ATTRIBUTION (SIGNIFICANT SHIFTS)")
        lines.append("-" * 125)
        shifts = []
        for r in self.results["flag_results"]:
            if r.get("status") == "PASS" and abs(r.get("delta_ms", 0.0)) >= 0.20:
                shifts.append(r)

        if not shifts:
            lines.append("  No flags exhibited wall time delta >= 0.20 ms.")
        else:
            p_hdr = f"{'Flag':<42} {'GPU Tot':>8} {'March':>7} {'March2':>7} {'Resolve':>8} {'Hi-Z':>6} {'TAA':>6} {'Shaft':>6}"
            lines.append(p_hdr)
            lines.append("-" * 125)
            for s in shifts:
                p = s.get("passes") or {}
                flag_disp = s['flag']
                if len(flag_disp) > 41:
                    flag_disp = flag_disp[:38] + "..."
                lines.append(
                    f"{flag_disp:<42} "
                    f"{s.get('gpu_total_ms', 0.0):>7.2f}m "
                    f"{p.get('march', 0.0):>6.2f}m "
                    f"{p.get('march2', 0.0):>6.2f}m "
                    f"{p.get('resolve', 0.0):>7.2f}m "
                    f"{p.get('hiz', 0.0):>5.2f}m "
                    f"{p.get('taa', 0.0):>5.2f}m "
                    f"{p.get('shaft', 0.0):>5.2f}m"
                )
        lines.append("")

        # Section 5: Faults & Optimization Targets
        lines.append("[5/5] FAULTS, ANOMALIES & OPTIMIZATION TARGETS")
        lines.append("-" * 110)
        if self.results["anomalies"]:
            lines.append("  [!] DETECTED ANOMALIES & INVARIANT FAILURES:")
            for a in self.results["anomalies"]:
                lines.append(f"      - {a}")
        else:
            lines.append("  [+] No unexpected crashes or invariant drifts detected.")

        # Top performance drains
        valid_passes = [r for r in self.results["flag_results"] if r.get("status") == "PASS"]
        if valid_passes:
            lines.append("\n  [+] SUBSYSTEM COST RANKING (Cost when active vs when disabled):")
            disabled_flags = [r for r in valid_passes if r["flag"].startswith("--no-")]
            disabled_flags.sort(key=lambda x: x["speedup_pct"], reverse=True)
            for d in disabled_flags[:6]:
                lines.append(
                    f"      * Disabling '{d['flag']}' saves {abs(d['delta_ms']):.2f} ms ({d['speedup_pct']:+.1f}% speedup) -> Primary target for optimization."
                )

        lines.append("\n" + "=" * 110)
        lines.append(" END OF AUDIT REPORT — SEND THIS FILE BACK TO ASSISTANT FOR OPTIMIZATION")
        lines.append("=" * 110)

        report_content = "\n".join(lines)
        with open(self.output_file, "w", encoding="utf-8") as f:
            f.write(report_content)

        print(f"\n[+] Audit complete! Report successfully written to: {self.output_file.resolve()}")
        print("    You can now send 'uploadme.txt' back to inspect faults and guide optimizations.")


def main():
    parser = argparse.ArgumentParser(description="VoxelCraft Flag-by-Flag Benchmark & Regression Auditor")
    parser.add_argument("--frames", type=int, default=60, help="Number of benchmark frames per run (default: 60)")
    parser.add_argument("--width", type=int, default=1280, help="Capture width (default: 1280)")
    parser.add_argument("--height", type=int, default=720, help="Capture height (default: 720)")
    parser.add_argument("--quick", action="store_true", help="Run quick audit on a curated subset of key flags")
    parser.add_argument("--category", type=str, default=None, help="Filter by category (e.g. 'water', 'shadows')")
    parser.add_argument("--flags", type=str, default=None, help="Comma-separated custom flag list")
    parser.add_argument("--bin", type=str, default=None, help="Custom binary path or runner command")
    parser.add_argument("--output", type=str, default="uploadme.txt", help="Output report path (default: uploadme.txt)")
    parser.add_argument("--skip-cargo-test", action="store_true", help="Skip cargo test execution")
    parser.add_argument("--dry-run", action="store_true", help="Simulate execution without requiring GPU / Cargo")

    args = parser.parse_args()
    auditor = VoxelCraftAuditor(args)
    if args.dry_run:
        auditor.run_simulation()
    else:
        auditor.prepare()
        auditor.run_system_verification()
        auditor.run_baseline()
        auditor.run_flag_matrix()
        auditor.write_report()


if __name__ == "__main__":
    main()
