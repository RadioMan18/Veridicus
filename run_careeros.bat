@echo off
setlocal enabledelayedexpansion

rem CareerOS launcher.
rem   run_careeros.bat             launch (release build if present, else dev mode)
rem   run_careeros.bat --rebuild   recompile the release binary, then launch
rem Works from any directory, including double-clicking this file.

cd /d "%~dp0"

echo ========================================
echo CareerOS Desktop Launcher
echo ========================================
echo.

if not exist "src-tauri\Cargo.toml" (
echo [ERROR] Must be run from the CareerOS root directory
echo         Current directory: %CD%
pause
exit /b 1
)

rem --- Environment -------------------------------------------------------
rem SQLCipher links against a system OpenSSL install. src-tauri\.cargo\config.toml
rem carries these values for cargo runs started inside src-tauri; setting them
rem here as well keeps builds working from any working directory.
if not defined OPENSSL_DIR (
if exist "C:\Program Files\OpenSSL-Win64\include\openssl\ssl.h" (
set "OPENSSL_DIR=C:\Program Files\OpenSSL-Win64"
set "OPENSSL_LIB_DIR=C:\Program Files\OpenSSL-Win64\lib\VC\x64\MD"
set "OPENSSL_INCLUDE_DIR=C:\Program Files\OpenSSL-Win64\include"
)
)
set "OPENSSL_NO_VENDOR=1"

if defined OPENSSL_DIR (
echo [INFO] OpenSSL: !OPENSSL_DIR!
) else (
echo [WARN] OpenSSL not found in "C:\Program Files\OpenSSL-Win64"
echo        Release builds need it. Set OPENSSL_DIR and rerun.
)

set "LOG=%TEMP%\careeros_launcher.log"
set "EXE=src-tauri\target\release\careeros.exe"
set "REBUILD=0"
if /i "%~1"=="--rebuild" set "REBUILD=1"
if /i "%~1"=="-r" set "REBUILD=1"

if "!REBUILD!"=="1" (
echo.
echo [INFO] Rebuilding the release binary ^(a few minutes^)...
pushd src-tauri
cargo build --release
set "BUILD_EXIT=!ERRORLEVEL!"
popd
if not "!BUILD_EXIT!"=="0" (
echo [ERROR] Release build failed with exit code !BUILD_EXIT!
pause
exit /b 1
)
)

if exist "!EXE!" (
powershell -NoProfile -Command "$exe = Get-Item 'src-tauri\target\release\careeros.exe'; $newer = (Get-ChildItem 'src','src-tauri\src' -Recurse -Include *.tsx,*.ts,*.css,*.rs -ErrorAction SilentlyContinue | Where-Object { $_.LastWriteTime -gt $exe.LastWriteTime }).Count; if ($newer -gt 0) { Write-Host ('[WARN] ' + $newer + ' source file(s) changed since this build; run run_careeros.bat --rebuild to include them.') }"
)

taskkill /F /IM careeros.exe > "%LOG%" 2>&1

if exist "!EXE!" (
echo.
echo [INFO] Starting CareerOS from the release build...
start "" "%~dp0!EXE!"
echo [SUCCESS] CareerOS launched
exit /b 0
)

echo [INFO] No release build found. Starting development mode...
echo [INFO] Compiling on first run can take several minutes.
echo.

where npm > "%LOG%" 2>&1
if errorlevel 1 (
echo [ERROR] npm not found in PATH
pause
exit /b 1
)

if not exist "node_modules" (
echo [INFO] Installing npm dependencies...
call npm install
if errorlevel 1 (
echo [ERROR] npm install failed
pause
exit /b 1
)
)

echo [INFO] Running: npm run tauri dev
echo.
call npm run tauri dev
if errorlevel 1 (
echo.
echo [ERROR] Failed to start CareerOS
pause
exit /b 1
)

exit /b 0
