; ================================================================
; NOVA — NSIS 安装脚本（per-user 模式，无需管理员权限）
; ================================================================
; 文件: NOVA-installer.nsi
; 说明: NOVA 桌面应用的 NSIS 安装器和卸载器
; 权限: per-user（HKCU + %LOCALAPPDATA% + 用户级开始菜单）
;
; 需要插件:
;   - EnVar 插件（用于 PATH 环境变量操作）
;     下载: https://nsis.sourceforge.io/EnVar_plug-in
;     放在 NSIS Plugins 目录 (x86-unicode/Plugins)
;
; 构建命令:
;   makensis /DAPP_VERSION=1.0.0 NOVA-installer.nsi
;
; 数据目录映射（与 Tauri/Rust 代码一致）:
;   安装目录:       $LOCALAPPDATA\NOVA
;   应用数据:       $LOCALAPPDATA\com.tinyzhuang.tokenicode
;   安全数据(设置): %USERPROFILE%\.tokenicode
;   Claude 项目:    %USERPROFILE%\.claude\projects\  (不删除)
;   WebView2 数据:  %APPDATA%\com.nova.app\
;   WebView2 Runtime: 系统级安装，不删除
; ================================================================

; --- 基本配置 ---
Unicode true
ManifestDPIAware true

!define APP_NAME "NOVA"
!define APP_EXE "NOVA.exe"
!define APP_CLI_EXE "claude.exe"
!define APP_PUBLISHER "NOVA Team"
!define APP_URL "https://github.com/yiliqi78/TOKENICODE"
!define APP_DATA_DIR "com.tinyzhuang.tokenicode"
!define APP_REG_KEY "Software\${APP_PUBLISHER}\${APP_NAME}"

; ================================================================
; 安装器部分
; ================================================================

Name "${APP_NAME}"
OutFile "NOVA-Setup.exe"

; per-user: 安装到 %LOCALAPPDATA%\NOVA，无需管理员权限
RequestExecutionLevel user
InstallDir "$LOCALAPPDATA\${APP_NAME}"

; UI 设置
!include "MUI2.nsh"
!define MUI_ABORTWARNING
!define MUI_ICON "icon.ico"
!define MUI_UNICON "icon.ico"

; 语言
!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

; 安装页面
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "LICENSE.txt"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

; 卸载页面
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH

; ================================================================
; 安装 Section
; ================================================================
Section "Install"

    SetOutPath "$INSTDIR"

    ; 1. 复制应用文件（由 Tauri 构建提供）
    File /r "bundle\*"

    ; 2. 写入注册表（HKCU — 卸载信息）
    WriteRegStr HKCU "${APP_REG_KEY}" "InstallDir" "$INSTDIR"
    WriteRegStr HKCU "${APP_REG_KEY}" "Version" "${APP_VERSION}"
    WriteRegStr HKCU "${APP_REG_KEY}" "DisplayName" "${APP_NAME}"
    WriteRegStr HKCU "${APP_REG_KEY}" "Publisher" "${APP_PUBLISHER}"
    WriteRegStr HKCU "${APP_REG_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
    WriteRegStr HKCU "${APP_REG_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
    WriteRegStr HKCU "${APP_REG_KEY}" "DisplayIcon" '"$INSTDIR\${APP_EXE}"'
    WriteRegStr HKCU "${APP_REG_KEY}" "URLInfoAbout" "${APP_URL}"
    WriteRegDWORD HKCU "${APP_REG_KEY}" "NoModify" 1
    WriteRegDWORD HKCU "${APP_REG_KEY}" "NoRepair" 1

    ; 计算安装大小（KB）
    ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
    IntFmt $0 "0x%08X" $0
    WriteRegDWORD HKCU "${APP_REG_KEY}" "EstimatedSize" "$0"

    ; 3. 添加到用户级 PATH 环境变量（使用 EnVar 插件）
    ;    CLI bin 目录: %LOCALAPPDATA%\com.tinyzhuang.tokenicode\cli
    ;    这样用户可以在命令行直接运行 claude
    StrCpy $0 "$LOCALAPPDATA\${APP_DATA_DIR}\cli"
    ${If} ${FileExists} "$0\${APP_CLI_EXE}"
        EnVar::SetHKCU
        EnVar::AddValueEx "PATH" "$0"
        Pop $1  ; 返回值 (0=成功, 非0=失败)
        ${If} $1 != 0
            ; PATH 添加失败，记录但不中断安装
            DetailPrint "警告: 无法添加 CLI 路径到用户 PATH ($1)"
        ${EndIf}
    ${EndIf}

    ; 4. 创建开始菜单快捷方式（用户级）
    CreateDirectory "$SMPROGRAMS\${APP_NAME}"
    CreateShortcut "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}" "" "$INSTDIR\${APP_EXE}" 0
    CreateShortcut "$SMPROGRAMS\${APP_NAME}\卸载 NOVA.lnk" "$INSTDIR\uninstall.exe" "" "$INSTDIR\uninstall.exe" 0

    ; 5. 写入卸载器（复制自身为 uninstall.exe）
    WriteUninstaller "$INSTDIR\uninstall.exe"

    ; 6. 创建用于「添加/删除程序」的注册表项
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" \
        "DisplayName" "${APP_NAME}"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" \
        "UninstallString" '"$INSTDIR\uninstall.exe"'
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" \
        "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" \
        "DisplayIcon" '"$INSTDIR\${APP_EXE}"'
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" \
        "Publisher" "${APP_PUBLISHER}"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" \
        "DisplayVersion" "${APP_VERSION}"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" \
        "URLInfoAbout" "${APP_URL}"
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" \
        "NoModify" 1
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" \
        "NoRepair" 1
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" \
        "EstimatedSize" "$0"

SectionEnd

; ================================================================
; 卸载器 Section
; ================================================================
Section "Uninstall"

    ; ---- 步骤 1: 停止 NOVA 进程 ----
    DetailPrint "正在停止 NOVA 进程..."
    nsExec::ExecToStack 'taskkill /F /IM ${APP_EXE}'
    Pop $0  ; 返回码
    ${If} $0 != 0
        ; 进程可能已经不在运行，这是正常的
        DetailPrint "NOVA 进程未在运行或已停止"
    ${Else}
        DetailPrint "已终止 NOVA 进程"
        Sleep 1500  ; 等待进程完全退出
    ${EndIf}

    ; 也尝试终止 claude CLI 进程
    nsExec::ExecToStack 'taskkill /F /IM ${APP_CLI_EXE}'
    Sleep 800

    ; ---- 步骤 2: 尝试删除安装目录 ----
    DetailPrint "正在清理安装目录: $INSTDIR"

    ; 先删除 uninstall.exe 自身以外的主要文件
    Delete "$INSTDIR\${APP_EXE}"
    Delete "$INSTDIR\*.dll"
    Delete "$INSTDIR\*.exe"
    Delete "$INSTDIR\*.dat"
    Delete "$INSTDIR\*.pak"
    Delete "$INSTDIR\*.bin"

    ; 删除所有子目录和文件
    RMDir /r "$INSTDIR\locales"
    RMDir /r "$INSTDIR\swiftshader"
    RMDir /r "$INSTDIR\resources"
    RMDir /r "$INSTDIR\tools"
    Delete "$INSTDIR\*.*"

    ; 尝试删除安装目录本身
    RMDir "$INSTDIR"
    ${If} ${FileExists} "$INSTDIR"
        ; 目录未完全删除（可能有残留文件/进程占用）
        ; 标记为重启后删除
        DetailPrint "警告: 安装目录未完全清理，部分文件将在系统重启后删除"
        Delete /REBOOTOK "$INSTDIR\*.*"
        RMDir /REBOOTOK "$INSTDIR"
    ${EndIf}

    ; ---- 步骤 3: 调用独立清理工具处理用户数据 ----
    ;    清理: %LOCALAPPDATA%\com.tinyzhuang.tokenicode
    ;    清理: %APPDATA%\com.nova.app (WebView2 数据)
    ;    保留: %USERPROFILE%\.tokenicode (用户设置)
    ;    保留: %USERPROFILE%\.claude\projects (Claude 项目数据，用户没要求删)
    DetailPrint "正在清理用户数据目录..."

    ; 检查独立的 cleanup-helper.exe 是否存在
    ${If} ${FileExists} "$INSTDIR\cleanup-helper.exe"
        ; 使用独立 helper
        nsExec::ExecToLog '"$INSTDIR\cleanup-helper.exe"'
        Pop $0
    ${Else}
        ; 独立 helper 不存在，内联清理
        DetailPrint "使用内联清理..."

        ; 清理应用数据目录
        RMDir /r "$LOCALAPPDATA\${APP_DATA_DIR}"

        ; 清理 WebView2 用户数据
        RMDir /r "$APPDATA\com.nova.app"

        ; 清理可能的 Edge WebView 数据目录
        RMDir /r "$LOCALAPPDATA\com.nova.claude-code"
    ${EndIf}

    ; 删除 cleanup-helper 自身
    Delete "$INSTDIR\cleanup-helper.exe"

    ; ---- 步骤 4: 还原 PATH 环境变量 ----
    ;    仅移除 CLI 路径，不损坏其他条目
    DetailPrint "正在清理环境变量 PATH..."
    StrCpy $0 "$LOCALAPPDATA\${APP_DATA_DIR}\cli"
    EnVar::SetHKCU
    EnVar::DeleteValue "PATH" "$0"
    Pop $1
    ${If} $1 != 0
        DetailPrint "PATH 清理完成（或路径未在 PATH 中）"
    ${EndIf}

    ; ---- 步骤 5: 删除开始菜单快捷方式 ----
    DetailPrint "正在删除开始菜单快捷方式..."
    Delete "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk"
    Delete "$SMPROGRAMS\${APP_NAME}\卸载 NOVA.lnk"
    RMDir "$SMPROGRAMS\${APP_NAME}"

    ; ---- 步骤 6: 清理注册表 ----
    DetailPrint "正在清理注册表..."
    DeleteRegKey HKCU "${APP_REG_KEY}"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}"

SectionEnd

; ================================================================
; 卸载器初始化: 检测残留并提示
; ================================================================
Function un.onInit
    ; 检查 NOVA 进程是否在运行
    nsExec::ExecToStack 'tasklist /FI "IMAGENAME eq ${APP_EXE}" /NH'
    Pop $0
    Pop $1
    ${If} $0 == 0
        StrLen $2 $1
        ${If} $2 > 5
            ; 进程确实在运行
            MessageBox MB_YESNO|MB_ICONQUESTION \
                "检测到 NOVA 仍在运行。$\n$\n是否强制关闭 NOVA 并继续卸载？" \
                IDYES +2
            Quit
        ${EndIf}
    ${EndIf}

    ; 检查是否是静默卸载 (/S)
    ${GetOptions} $CMDLINE "/S" $0
    IfErrors 0 +2
        SetSilent silent
FunctionEnd

; ================================================================
; 卸载结果回调: 即使部分失败也标记成功（重复卸载兼容）
; ================================================================
Function un.onUninstSuccess
    ; 总是隐藏卸载成功提示（静默）
    ${If} ${Silent}
        ; 静默卸载不弹窗
    ${EndIf}
FunctionEnd

; ================================================================
; 版本信息
; ================================================================
VIProductVersion "${APP_VERSION}.0"
VIAddVersionKey "ProductName" "${APP_NAME}"
VIAddVersionKey "CompanyName" "${APP_PUBLISHER}"
VIAddVersionKey "FileDescription" "${APP_NAME} Installer"
VIAddVersionKey "LegalCopyright" "${APP_PUBLISHER}"
VIAddVersionKey "FileVersion" "${APP_VERSION}"
