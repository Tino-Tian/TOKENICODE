@echo off
REM ============================================================
REM NOVA cleanup-helper 构建脚本 (Windows)
REM ============================================================
REM 前置条件: 安装 Go 1.21+ (https://go.dev/dl/)
REM
REM 构建命令:
REM   build.bat
REM
REM 输出:
REM   cleanup-helper.exe  (静态编译，无依赖，~2MB)
REM ============================================================

echo === NOVA cleanup-helper 构建 ===
echo.

go build -ldflags "-s -w -H windowsgui" -o cleanup-helper.exe main.go

if %ERRORLEVEL% EQU 0 (
    echo.
    echo 构建成功: cleanup-helper.exe
    echo 大小:
    dir cleanup-helper.exe | find "cleanup-helper.exe"
    echo.
    echo 将此文件放置到 NSIS 安装目录下即可。
) else (
    echo.
    echo 构建失败，请检查 Go 安装。
)
