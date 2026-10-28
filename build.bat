@echo off
rem Release build: produces target\release\kanvibe.exe. Run from anywhere.
setlocal
cd /d "%~dp0"

cargo build --release || goto :fail

echo Built target\release\kanvibe.exe
exit /b 0

:fail
echo BUILD FAILED.
exit /b 1
