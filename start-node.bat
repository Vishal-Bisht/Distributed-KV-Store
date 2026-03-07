@echo off
REM Start a KV store node
REM Usage: start-node.bat <node_id> [port]
REM Example: start-node.bat 1
REM          start-node.bat 2 8002

set ID=%1
set PORT=%2

if "%ID%"=="" (
    echo Usage: start-node.bat ^<node_id^> [port]
    echo Example: start-node.bat 1
    exit /b 1
)

if "%PORT%"=="" set PORT=800%ID%

REM Default 3-node cluster on localhost
if "%ID%"=="1" (
    .\target\release\kv.exe -i 1 -a 127.0.0.1:8001 -p 127.0.0.1:8002,127.0.0.1:8003
) else if "%ID%"=="2" (
    .\target\release\kv.exe -i 2 -a 127.0.0.1:8002 -p 127.0.0.1:8001,127.0.0.1:8003
) else if "%ID%"=="3" (
    .\target\release\kv.exe -i 3 -a 127.0.0.1:8003 -p 127.0.0.1:8001,127.0.0.1:8002
) else (
    echo Invalid node ID. Use 1, 2, or 3
)
