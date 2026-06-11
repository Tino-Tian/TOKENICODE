#!/usr/bin/env bash
# ============================================================
# NOVA — U 盘分发包组装脚本（macOS 上运行）
# ============================================================
# 用途: 将 CI 下载的 NSIS 安装程序和配套脚本
#       打包成一个完整的 U 盘分发包目录。
# 用法: ./assemble-usb.sh [版本号]
#       不传版本号则默认使用 1.0.0-beta
# ============================================================

set -euo pipefail

# ── 配置 ──────────────────────────────────────────────
VERSION="${1:-1.0.0-beta}"
APP_NAME="NOVA"
PLATFORM="windows-x64"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
DIST_DIR="$PROJECT_DIR/dist/usb/${APP_NAME}-v${VERSION}-${PLATFORM}"
NSIS_EXE="$PROJECT_DIR/dist/nsis/${APP_NAME}-v${VERSION}-${PLATFORM}-setup.exe"
SCRIPTS_SRC="$SCRIPT_DIR"

# 颜色
RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo ""
echo -e "${CYAN}============================================${NC}"
echo -e "${CYAN}  NOVA — U 盘分发包组装工具${NC}"
echo -e "${CYAN}============================================${NC}"
echo ""
echo -e "版本号:   ${GREEN}${VERSION}${NC}"
echo -e "目标平台: ${GREEN}${PLATFORM}${NC}"
echo -e "输出目录: ${GREEN}${DIST_DIR}${NC}"
echo ""

# ── 步骤 1: 检查 NSIS .exe 是否存在 ─────────────────
echo -e "${CYAN}[1/4]${NC} 检查 NSIS 安装程序..."

if [[ ! -f "$NSIS_EXE" ]]; then
    echo ""
    echo -e "${RED}❌ 错误:${NC} 未找到 NSIS 安装程序:"
    echo -e "   ${NSIS_EXE}"
    echo ""
    echo "   请先运行 NSIS 构建流程（如 'npm run build:nsis'）"
    echo "   再将生成的 .exe 放到 dist/nsis/ 目录下。"
    echo ""
    exit 1
fi

EXE_SIZE=$(du -h "$NSIS_EXE" | cut -f1)
echo -e "  ${GREEN}✅${NC} 找到安装程序: $(basename "$NSIS_EXE") (${EXE_SIZE})"

# ── 步骤 2: 创建目录并复制文件 ──────────────────────
echo -e "${CYAN}[2/4]${NC} 创建 U 盘包目录并复制文件..."

# 清理旧目录（如果存在）
if [[ -d "$DIST_DIR" ]]; then
    echo "  清理旧输出目录..."
    rm -rf "$DIST_DIR"
fi

mkdir -p "$DIST_DIR"

# 复制 NSIS 安装程序
cp "$NSIS_EXE" "$DIST_DIR/"
echo -e "  ${GREEN}✅${NC} 复制安装程序"

# 复制诊断脚本
if [[ -f "$SCRIPTS_SRC/diagnostics.ps1" ]]; then
    cp "$SCRIPTS_SRC/diagnostics.ps1" "$DIST_DIR/"
    echo -e "  ${GREEN}✅${NC} 复制系统诊断脚本 (diagnostics.ps1)"
else
    echo -e "  ${YELLOW}⚠️${NC}  诊断脚本未找到: $SCRIPTS_SRC/diagnostics.ps1"
fi

# 复制 WebView2 离线安装脚本
if [[ -f "$SCRIPTS_SRC/install-webview2-offline.cmd" ]]; then
    cp "$SCRIPTS_SRC/install-webview2-offline.cmd" "$DIST_DIR/"
    echo -e "  ${GREEN}✅${NC} 复制 WebView2 安装脚本 (install-webview2-offline.cmd)"
else
    echo -e "  ${YELLOW}⚠️${NC}  WebView2 安装脚本未找到: $SCRIPTS_SRC/install-webview2-offline.cmd"
fi

# 创建使用说明
cat > "$DIST_DIR/使用说明.txt" << 'INSTRUCTIONS'
============================================
  NOVA — 安装使用说明
============================================

【安装前检查】
  1. 双击运行 "diagnostics.ps1"
     如果无法运行，右键 → "使用 PowerShell 运行"
  2. 查看诊断报告，确认所有项为 ✅ 通过
  3. 如 WebView2 项显示 ❌，运行 "install-webview2-offline.cmd"

【开始安装】
  1. 双击 "NOVA-vX.X.X-windows-x64-setup.exe"
  2. 按照安装向导完成安装
  3. 安装完成后桌面会出现 NOVA 快捷方式

【常见问题】
  Q: 双击 .ps1 文件用记事本打开了？
  A: 右键文件 → "使用 PowerShell 运行"，或在 PowerShell 中输入:
     cd "U盘盘符:\本目录"
     .\diagnostics.ps1

  Q: 提示"无法加载，因为在此系统上禁止运行脚本"？
  A: 以管理员身份打开 PowerShell，执行:
     Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
     然后重新运行 .ps1 脚本。

  Q: WebView2 安装失败？
  A: 确保系统已联网，然后重试。如仍失败，访问:
     https://developer.microsoft.com/microsoft-edge/webview2/

【技术支持】
  访问 NOVA 项目主页获取最新信息和帮助。
============================================
INSTRUCTIONS

echo -e "  ${GREEN}✅${NC} 创建使用说明 (使用说明.txt)"

# ── 步骤 3: 生成 SHA256 校验文件 ────────────────────
echo -e "${CYAN}[3/4]${NC} 生成 SHA256 校验文件..."

cd "$DIST_DIR"

# 对所有文件生成校验（排除校验文件自身和目录）
shasum -a 256 * 2>/dev/null > "SHA256SUMS" || true

echo -e "  ${GREEN}✅${NC} 已生成 SHA256SUMS"
echo ""
echo "  校验值预览:"
while IFS= read -r line; do
    checksum=$(echo "$line" | awk '{print $1}')
    filename=$(echo "$line" | awk '{print $2}')
    if [[ -n "$filename" && "$filename" != "SHA256SUMS" ]]; then
        printf "    %s  %s\n" "$checksum" "$filename"
    fi
done < SHA256SUMS

# ── 步骤 4: 打印目录结构 ────────────────────────────
echo ""
echo -e "${CYAN}[4/4]${NC} U 盘包目录结构:"
echo -e "${CYAN}────────────────────────────────────────────${NC}"

# 用 tree（如果有）或 find 展示
if command -v tree &> /dev/null; then
    tree -h "$DIST_DIR" --du 2>/dev/null || tree -h "$DIST_DIR"
else
    find "$DIST_DIR" -maxdepth 1 -not -name '.' -not -name '..' | sort | while read -r item; do
        if [[ -f "$item" ]]; then
            SIZE=$(du -h "$item" | cut -f1)
            printf "  %6s  %s\n" "$SIZE" "$(basename "$item")"
        elif [[ -d "$item" ]]; then
            printf "  %6s  %s/\n" "-" "$(basename "$item")"
        fi
    done
fi

echo -e "${CYAN}────────────────────────────────────────────${NC}"

# ── 底部总结 ─────────────────────────────────────────
TOTAL_SIZE=$(du -sh "$DIST_DIR" 2>/dev/null | cut -f1)
FILE_COUNT=$(find "$DIST_DIR" -maxdepth 1 -type f | wc -l | tr -d ' ')

echo ""
echo -e "${GREEN}============================================${NC}"
echo -e "${GREEN}  ✅ U 盘分发包组装完成！${NC}"
echo -e "${GREEN}============================================${NC}"
echo ""
echo -e "  输出目录: ${DIST_DIR}"
echo -e "  文件数量: ${FILE_COUNT} 个"
echo -e "  总大小:   ${TOTAL_SIZE}"
echo ""
echo -e "  将此目录下的所有文件复制到 U 盘根目录即可。"
echo ""
