@echo off
chcp 65001 >nul
setlocal enabledelayedexpansion

echo ========================================
echo   NOVA - WebView2 离线安装脚本
echo ========================================
echo.
echo 正在检测 WebView2 是否已安装...
echo.

rem 方式一：检查注册表（独立安装版和 Edge 内置版共用更新通道）
reg query "HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" >nul 2>&1
if %errorlevel% equ 0 (
    echo ✅ WebView2 已安装，无需重复安装。
    echo.
    pause
    exit /b 0
)

rem 方式二：检查文件目录是否存在
if exist "%ProgramFiles(x86)%\Microsoft\WebView2\" (
    echo ✅ WebView2 已安装（检测到程序目录），无需重复安装。
    echo.
    pause
    exit /b 0
)

if exist "%ProgramFiles%\Microsoft\WebView2\" (
    echo ✅ WebView2 已安装（检测到程序目录），无需重复安装。
    echo.
    pause
    exit /b 0
)

if exist "%LOCALAPPDATA%\Microsoft\EdgeWebView\Application\" (
    echo ✅ WebView2 已安装（检测到程序目录），无需重复安装。
    echo.
    pause
    exit /b 0
)

echo ⚠️  WebView2 未安装，正在准备离线安装...
echo.

rem 查找安装程序（先看同目录，再看子目录）
set "SETUP_EXE="
if exist "%~dp0MicrosoftEdgeWebview2Setup.exe" (
    set "SETUP_EXE=%~dp0MicrosoftEdgeWebview2Setup.exe"
)
if "!SETUP_EXE!"=="" (
    if exist "%~dp0redist\MicrosoftEdgeWebview2Setup.exe" (
        set "SETUP_EXE=%~dp0redist\MicrosoftEdgeWebview2Setup.exe"
    )
)
if "!SETUP_EXE!"=="" (
    if exist "%~dp0..\redist\MicrosoftEdgeWebview2Setup.exe" (
        set "SETUP_EXE=%~dp0..\redist\MicrosoftEdgeWebview2Setup.exe"
    )
)

if not "!SETUP_EXE!"=="" (
    echo 找到安装程序: !SETUP_EXE!
    echo 正在静默安装，请稍候（可能需要几分钟）...
    echo.
    "!SETUP_EXE!" /silent /install
    if !errorlevel! equ 0 (
        echo.
        echo ✅ WebView2 安装完成。
        echo 请重新运行 diagnostics.ps1 验证安装，或直接继续安装 NOVA。
    ) else (
        echo.
        echo ❌ 安装失败，错误代码: !errorlevel!
        echo.
        echo 请尝试以下方式手动安装：
        echo   1. 检查系统是否联网
        echo   2. 访问 https://developer.microsoft.com/microsoft-edge/webview2/
        echo   3. 下载"Evergreen Bootstrapper"或"Evergreen Standalone Installer"
    )
) else (
    echo ❌ 未找到 MicrosoftEdgeWebview2Setup.exe
    echo.
    echo 请从 Microsoft 官网下载 WebView2 运行时：
    echo   https://developer.microsoft.com/microsoft-edge/webview2/
    echo.
    echo 下载说明：
    echo   - 在线环境：下载"Evergreen Bootstrapper（引导程序）"
    echo   - 离线环境：下载"Evergreen Standalone Installer（独立安装程序）"
    echo.
    echo   将下载的文件命名为 MicrosoftEdgeWebview2Setup.exe
    echo   放在与本脚本相同的目录下，然后重新运行本脚本。
)

echo.
pause
