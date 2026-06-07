#!/bin/bash
# NOVA 便携版构建脚本
# 构建完成后，将 dist/portable/NOVA/ 整个文件夹复制到 U 盘即可

set -e

echo "=== NOVA 便携版构建 ==="

# 构建 Tauri 应用
npm run tauri build

# 创建便携输出目录
mkdir -p dist/portable/NOVA

# 复制构建产物
if [ -d "src-tauri/target/release/bundle/macos" ]; then
  cp -r src-tauri/target/release/bundle/macos/* dist/portable/NOVA/
fi
if [ -d "src-tauri/target/release/bundle/msi" ]; then
  cp -r src-tauri/target/release/bundle/msi/* dist/portable/NOVA/
fi
if [ -d "src-tauri/target/release/bundle/nsis" ]; then
  cp -r src-tauri/target/release/bundle/nsis/* dist/portable/NOVA/
fi

# 复制说明文件
cp 使用指南.txt dist/portable/
cp DeepSeek_API_申请教程.md dist/portable/NOVA/

echo "=== 构建完成 ==="
echo "便携版位置: dist/portable/"
echo "将整个 NOVA 文件夹复制到 U 盘即可分发给朋友"
