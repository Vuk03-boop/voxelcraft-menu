#!/usr/bin/env bash
set -e

echo "=========================================================="
echo " Starting VoxelCraft Performance & Regression Audit Suite "
echo "=========================================================="

# 1. Check for Python 3
if ! command -v python3 &> /dev/null; then
    echo "[!] Error: python3 is required to run the audit suite."
    exit 1
fi

# 2. Pre-compile binary if target does not exist (speeds up test execution)
if command -v cargo &> /dev/null; then
    if [ ! -f "target/release/voxelcraft" ] && [ ! -f "target/release/voxelcraft.exe" ]; then
        echo "[*] Pre-compiling VoxelCraft release binary for maximum sweep speed..."
        cargo build --release
    fi
fi

# 3. Launch audit suite
python3 run_audit_suite.py "$@"

echo ""
echo "=========================================================="
echo " Done! Results are compiled in 'uploadme.txt'"
echo " Please upload or send 'uploadme.txt' back to the assistant."
echo "=========================================================="
