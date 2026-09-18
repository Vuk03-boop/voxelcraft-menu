@echo off
echo ==========================================================
echo  Starting VoxelCraft Performance & Regression Audit Suite
echo ==========================================================

REM Check for Python
python --version >nul 2>&1
if %errorlevel% neq 0 (
    echo [!] Error: Python 3 was not found in PATH.
    pause
    exit /b 1
)

REM Pre-compile release binary if needed
if not exist "target\release\voxelcraft.exe" (
    where cargo >nul 2>&1
    if %errorlevel% equ 0 (
        echo [*] Pre-compiling VoxelCraft release binary for maximum sweep speed...
        cargo build --release
    )
)

python run_audit_suite.py %*

echo.
echo ==========================================================
echo  Done! Results are compiled in 'uploadme.txt'
echo  Please upload or send 'uploadme.txt' back to the assistant.
echo ==========================================================
