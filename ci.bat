@echo off
rem Local CI: the checks every merge must pass. Run from the repo root.
setlocal
cd /d "%~dp0"

echo [1/3] cargo fmt --check
cargo fmt --check || goto :fail

echo [2/3] cargo clippy
cargo clippy --all-targets -q -- -D warnings || goto :fail

echo [3/3] cargo test
cargo test -q || goto :fail

echo CI passed.
exit /b 0

:fail
echo CI FAILED.
exit /b 1
