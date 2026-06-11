; NOVA — NSIS 用户级 PATH 操作宏
; Bug 1 (P0): 安装时写入 CLI 目录到 HKCU\Environment\Path
; 策略：通过内联 PowerShell 操作注册表，避免 NSIS 字符串处理局限
;
; === 工程师 C 合并指引 ===
;
; 一、将本文件放到 src-tauri/nsis/ 目录
;
; 二、在主 .nsi 中找到 Tauri 的 NSIS 模板（通常是
;    src-tauri/target/<target>/nsis/installer.nsi 生成版，
;    或是 src-tauri/nsis/custom.nsi 自定义模板）。
;
; 三、在安装段的文件复制完成后(@SectionEnd 前)加入：
;
;     ; Bug 1: 写 CLI 目录到用户 PATH
;     !include "nova_path.nsh"
;     ${NOVA_AddToUserPath} "$LOCALAPPDATA\NOVA\cli"
;
; 四、在卸载段中加入：
;
;     ${NOVA_RemoveFromUserPath} "$LOCALAPPDATA\NOVA\cli"
;
; 五、如果用的是 Tauri 生成的 .nsi（非自定义模板），请在
;    tauri.conf.json 的 bundle.windows.nsis 中设置
;    "installerHook": "nsis/nova_installer_hook.nsh" 然后在 hook
;    文件中调用本宏。具体参考 Tauri NSIS hook 文档。
; =================================================================

!ifndef NOVA_PATH_NSH
!define NOVA_PATH_NSH

; ── 将目录添加到 HKCU\Environment\Path ──────────────────────
; 用法: ${NOVA_AddToUserPath} "C:\Users\xxx\AppData\Local\NOVA\cli"
!macro NOVA_AddToUserPath _DIR
  ; 使用 PowerShell 操作注册表（与 Rust 后端 finalize_cli_install_paths 一致）
  ; 为什么用 PowerShell 而不是 NSIS 内置的 WriteRegExpandStr？
  ;   1. NSIS 的 PATH 追加逻辑需要手动处理分号、去重、大小写——容易出错
  ;   2. PowerShell 的 [Environment]::SetEnvironmentVariable 处理了所有边界情况
  ;   3. 与 Rust 后端保持一致，便于统一排查问题
  nsExec::ExecToStack 'powershell -NoProfile -NonInteractive -Command "$old=[Environment]::GetEnvironmentVariable(\"Path\",\"User\"); if(-not $old){[Environment]::SetEnvironmentVariable(\"Path\",\"${_DIR}\",\"User\")}elseif(-not $old.Contains(\"${_DIR}\")){[Environment]::SetEnvironmentVariable(\"Path\",\"$old;${_DIR}\",\"User\")}"'
  Pop $0  ; 返回值 (0=成功)

  ${If} $0 != 0
    ; PowerShell 不可用时降级为直接写注册表
    ReadRegStr $1 HKCU "Environment" "Path"
    ${If} $1 == ""
      WriteRegExpandStr HKCU "Environment" "Path" "${_DIR}"
    ${Else}
      WriteRegExpandStr HKCU "Environment" "Path" "$1;${_DIR}"
    ${EndIf}
  ${EndIf}
!macroend

; ── 从 HKCU\Environment\Path 移除目录 ───────────────────────
; 用法: ${NOVA_RemoveFromUserPath} "C:\Users\xxx\AppData\Local\NOVA\cli"
!macro NOVA_RemoveFromUserPath _DIR
  nsExec::ExecToStack 'powershell -NoProfile -NonInteractive -Command "$old=[Environment]::GetEnvironmentVariable(\"Path\",\"User\"); if($old){$new=($old -split \";\" | Where-Object {$_ -ne \"${_DIR}\"}) -join \";\"; [Environment]::SetEnvironmentVariable(\"Path\",$new,\"User\")}"'
  Pop $0
!macroend

; ── 安装时推荐的调用位置 ────────────────────────────────────
; 注意：这个宏会被合并到 NSIS 脚本中。下面是工程师 C 需要在
; 安装和卸载脚本中分别调用的位置示例。
;
; === 安装段 (Section / -post) ===
;   DetailPrint "写入 CLI 目录到系统 PATH..."
;   ${NOVA_AddToUserPath} "$LOCALAPPDATA\NOVA\cli"
;
; === 卸载段 (un.Section / un.-post) ===
;   DetailPrint "清理 PATH 中的 CLI 目录..."
;   ${NOVA_RemoveFromUserPath} "$LOCALAPPDATA\NOVA\cli"
; =================================================================

!endif ; NOVA_PATH_NSH
