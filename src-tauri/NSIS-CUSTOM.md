# NSIS 自定义安装脚本说明

## 当前集成方式

NOVA 使用 Tauri 2 的 `installerHooks` 机制，通过 `/src-tauri/nsis/installer_hooks.nsh` 注入自定义逻辑。

### tauri.conf.json 配置

```json
"nsis": {
  "installerHooks": "nsis/installer_hooks.nsh",
  ...
}
```

### installer_hooks.nsh 包含的功能

- **Bug 1 修复**: PATH 环境变量操作（安装时写入 CLI 目录，卸载时移除）
- **Bug 7 修复**: cleanup-helper 集成（安装后复制到安装目录，卸载时调用清理用户数据）

## 完整自定义 NSIS 模板（保留备用）

`/src-tauri/nsis/NOVA-installer.nsi` 是一个完整的独立 NSIS 脚本，可用于脱离 Tauri 独立构建安装包。

### 何时使用

- 需要完全自定义安装界面（而非 Tauri 默认界面）
- 需要在 Tauri 不支持的地方做深度定制
- 作为独立的安装包分发方案

### 构建命令

```bash
makensis /DAPP_VERSION=1.0.0 NOVA-installer.nsi
```

### 依赖

- NSIS 3.x + MUI2
- EnVar 插件（用于 PATH 操作）
- 预构建的 bundle 目录（由 `npm run tauri build` 产生）

### 与 Tauri 默认构建的差异

1. Tauri 构建流程会自动处理 WebView2 安装、签名等
2. 独立 NSIS 脚本需要手动准备 bundle 目录并处理这些依赖
3. 独立脚本面向高级用户和特殊分发场景

## 文件清单

| 文件 | 用途 |
|------|------|
| `nsis/installer_hooks.nsh` | Tauri 2 installerHooks（**当前使用**） |
| `nsis/nova_path.nsh` | PATH 环境变量操作宏（被 hook 引用） |
| `nsis/NOVA-installer.nsi` | 完整独立 NSIS 脚本（**保留备用**） |
| `tools/cleanup-helper/` | Go 编写的卸载清理助手 |
