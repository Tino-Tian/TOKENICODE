; NOVA — Tauri 2 NSIS Installer Hooks
; 说明: Tauri 2 在 tauri.conf.json 中通过 installerHooks 配置引用此文件。
;       此文件中的宏会被 Tauri 自动载入生成的 installer.nsi 中。
; 集成: Bug 1 PATH 操作 (nova_path.nsh) + Bug 7 清理助手
; 日期: 2026-06-11
; ================================================================

; ── 引用 PATH 操作宏 ─────────────────────────────────────────────
!addincludedir "nsis"
!include "nova_path.nsh"

; ── 安装前 ───────────────────────────────────────────────────────
!macro NSIS_HOOK_PREINSTALL
  ; 安装前无需特殊操作
!macroend

; ── 安装后 ───────────────────────────────────────────────────────
!macro NSIS_HOOK_POSTINSTALL
  ; Bug 1: 将 CLI 目录写入用户级 PATH
  ; 首次安装时 CLI 文件还不存在（由 Rust 在首次启动时下载），
  ; 因此不检测文件存在，始终写入 PATH，确保路径就绪。
  StrCpy $0 "$LOCALAPPDATA\NOVA\cli"
  DetailPrint "正在将 CLI 路径添加到用户 PATH..."
  ${NOVA_AddToUserPath} "$0"

  ; Bug 7: 复制 cleanup-helper.exe 到安装目录（如果存在）
  ;   Tauri resources 会将 tools/ 目录打包到 bundle，所以
  ;   cleanup-helper.exe 应该在 tools\cleanup-helper\ 下
  ${If} ${FileExists} "$INSTDIR\tools\cleanup-helper\cleanup-helper.exe"
    DetailPrint "安装清理助手工具..."
    CopyFiles /SILENT "$INSTDIR\tools\cleanup-helper\cleanup-helper.exe" "$INSTDIR\cleanup-helper.exe"
  ${EndIf}
!macroend

; ── 卸载前 ───────────────────────────────────────────────────────
!macro NSIS_HOOK_PREUNINSTALL
  ; 停止 NOVA 进程（在卸载文件之前）
  DetailPrint "正在停止 NOVA 进程..."
  nsExec::ExecToStack 'taskkill /F /IM NOVA.exe'
  Pop $0
  ; 也尝试终止 claude CLI
  nsExec::ExecToStack 'taskkill /F /IM claude.exe'
  Sleep 1500
!macroend

; ── 卸载后 ───────────────────────────────────────────────────────
!macro NSIS_HOOK_POSTUNINSTALL
  ; Bug 1: 从用户 PATH 移除 CLI 目录
  DetailPrint "正在清理用户 PATH 中的 CLI 目录..."
  ${NOVA_RemoveFromUserPath} "$LOCALAPPDATA\NOVA\cli"

  ; Bug 7: 运行 cleanup-helper 清理用户数据
  DetailPrint "正在清理用户数据..."
  ${If} ${FileExists} "$INSTDIR\cleanup-helper.exe"
    nsExec::ExecToLog '"$INSTDIR\cleanup-helper.exe"'
  ${Else}
    ; 独立 helper 不存在时，内联清理
    RMDir /r "$LOCALAPPDATA\com.tinyzhuang.tokenicode"
    RMDir /r "$APPDATA\com.nova.app"
    RMDir /r "$LOCALAPPDATA\com.nova.claude-code"
  ${EndIf}

  ; 注册表清理交由 Tauri 默认行为处理
!macroend
