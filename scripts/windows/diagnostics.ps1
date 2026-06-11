<#
.SYNOPSIS
    NOVA 系统环境诊断脚本
.DESCRIPTION
    用于检测 Windows 系统是否满足 NOVA 安装和运行的最低要求。
    用户插入 U 盘后双击运行此脚本，即可看到各项兼容性检查结果。
.NOTES
    文件名: diagnostics.ps1
    编码:   UTF-8 with BOM (兼容 PowerShell 5.1)
    作者:   NOVA 团队
    版本:   1.0.0
#>

#Requires -Version 5.1

$ErrorActionPreference = "Continue"

# ============================================
# 配置项
# ============================================
$MinBuildNumber = 17763          # Windows 10 1809 (17763)
$MinPowerShellVersion = "5.1"
$MinNodeVersion = "18.0.0"
$RequiredDiskSpaceMB = 2048      # 2 GB
$MinDiskSpaceMB = 1024           # 1 GB 警告阈值

# ============================================
# 辅助函数
# ============================================
function Write-CheckResult {
    param(
        [string]$Item,
        [string]$Status,   # Pass, Warn, Fail
        [string]$Detail,
        [string]$Suggestion
    )

    $icon = @{
        Pass = "✅"
        Warn = "⚠️"
        Fail = "❌"
    }[$Status]

    $line = "$icon $Item"
    if ($Detail) {
        $line += " — $Detail"
    }
    Write-Host $line

    if ($Suggestion) {
        Write-Host "   👉 $Suggestion" -ForegroundColor Yellow
    }
    Write-Host ""
}

function Compare-Version {
    param([string]$Actual, [string]$Required)
    # 简单语义版本比较：将版本号拆成数字数组逐段比
    $aParts = $Actual -split '\.' | ForEach-Object { [int]$_ }
    $rParts = $Required -split '\.' | ForEach-Object { [int]$_ }
    $maxLen = [Math]::Max($aParts.Count, $rParts.Count)
    for ($i = 0; $i -lt $maxLen; $i++) {
        $a = if ($i -lt $aParts.Count) { $aParts[$i] } else { 0 }
        $r = if ($i -lt $rParts.Count) { $rParts[$i] } else { 0 }
        if ($a -gt $r) { return 1 }
        if ($a -lt $r) { return -1 }
    }
    return 0
}

# ============================================
# 输出头部
# ============================================
Write-Host ""
Write-Host "============================================" -ForegroundColor Cyan
Write-Host "  NOVA — 系统环境诊断报告" -ForegroundColor Cyan
Write-Host "============================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "检查时间: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')" -ForegroundColor Gray
Write-Host "计算机名: $env:COMPUTERNAME" -ForegroundColor Gray
Write-Host ""

# ============================================
# 1. 操作系统版本
# ============================================
try {
    $os = Get-CimInstance Win32_OperatingSystem -ErrorAction Stop
    $build = [int]$os.BuildNumber
    $caption = $os.Caption
    $arch = $os.OSArchitecture

    if ($build -ge $MinBuildNumber) {
        Write-CheckResult -Item "操作系统版本" -Status Pass -Detail "$caption ($arch), Build $build"
    } elseif ($build -ge 10240) {
        Write-CheckResult -Item "操作系统版本" -Status Warn -Detail "$caption ($arch), Build $build（最低要求: Build $MinBuildNumber）" `
            -Suggestion "您的 Windows 版本较低，NOVA 可能仍能运行但未充分测试。建议通过 Windows Update 升级到最新版本。"
    } else {
        Write-CheckResult -Item "操作系统版本" -Status Fail -Detail "$caption ($arch), Build $build（最低要求: Build $MinBuildNumber）" `
            -Suggestion "Windows 版本过低，请先升级到 Windows 10 1809 或更高版本。"
    }
} catch {
    Write-CheckResult -Item "操作系统版本" -Status Fail -Detail "无法获取系统信息: $_" `
        -Suggestion "请检查系统是否正常运行，然后重新运行本脚本。"
}

# ============================================
# 2. PowerShell 版本
# ============================================
try {
    $psVersion = $PSVersionTable.PSVersion.ToString()
    $compare = Compare-Version -Actual $psVersion -Required $MinPowerShellVersion
    if ($compare -ge 0) {
        Write-CheckResult -Item "PowerShell 版本" -Status Pass -Detail "v$psVersion"
    } else {
        Write-CheckResult -Item "PowerShell 版本" -Status Fail -Detail "v$psVersion（最低要求: v$MinPowerShellVersion）" `
            -Suggestion "请安装 Windows Management Framework 5.1 或更高版本。"
    }
} catch {
    Write-CheckResult -Item "PowerShell 版本" -Status Fail -Detail "无法检测版本" `
        -Suggestion "请确保 PowerShell 正常运行。"
}

# ============================================
# 3. WebView2 Runtime
# ============================================
$webview2Found = $false
$webview2Detail = ""

# 方式一：Edge WebView2 注册表（独立安装版和 Edge 内置版共用一个更新通道）
$webviewReg = Get-ItemProperty "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" -ErrorAction SilentlyContinue
if ($webviewReg) {
    # 尝试获取版本号
    try {
        $pv = $webviewReg.pv
        if ($pv) {
            $webview2Found = $true
            $webview2Detail = "版本 $pv"
        }
    } catch {}
}

# 方式二：检查 WebView2 运行时文件夹
if (-not $webview2Found) {
    $wv2Paths = @(
        "${env:ProgramFiles(x86)}\Microsoft\WebView2\*\EBWebView"
        "${env:ProgramFiles}\Microsoft\WebView2\*\EBWebView"
        "${env:LOCALAPPDATA}\Microsoft\EdgeWebView\Application"
    )
    foreach ($p in $wv2Paths) {
        if (Test-Path $p) {
            $webview2Found = $true
            $webview2Detail = "已安装（检测到文件目录）"
            break
        }
    }
}

if ($webview2Found) {
    Write-CheckResult -Item "WebView2 Runtime" -Status Pass -Detail $webview2Detail
} else {
    Write-CheckResult -Item "WebView2 Runtime" -Status Fail -Detail "未检测到 WebView2" `
        -Suggestion "请运行 U 盘中的 install-webview2-offline.cmd 进行离线安装。"
}

# ============================================
# 4. Node.js 版本
# ============================================
try {
    $nodeOutput = & node --version 2>&1
    if ($LASTEXITCODE -eq 0 -and $nodeOutput -match 'v?(\d+\.\d+\.\d+)') {
        $nodeVersion = $Matches[1]
        $compare = Compare-Version -Actual $nodeVersion -Required $MinNodeVersion
        if ($compare -ge 0) {
            Write-CheckResult -Item "Node.js" -Status Pass -Detail "v$nodeVersion"
        } else {
            Write-CheckResult -Item "Node.js" -Status Warn -Detail "v$nodeVersion（建议: v$MinNodeVersion+）" `
                -Suggestion "Node.js 版本较低，建议从 https://nodejs.org 下载安装 LTS 版本。"
        }
    } else {
        throw "无法解析版本"
    }
} catch [System.Management.Automation.CommandNotFoundException] {
    Write-CheckResult -Item "Node.js" -Status Fail -Detail "未安装" `
        -Suggestion "Node.js 是 NOVA 运行的必备环境。请从 https://nodejs.org 下载 LTS 版本并安装。"
} catch {
    Write-CheckResult -Item "Node.js" -Status Warn -Detail "检测异常: $_" `
        -Suggestion "请确认 Node.js 已正确安装并加入 PATH 环境变量。"
}

# ============================================
# 5. Git 可用性
# ============================================
try {
    $gitOutput = & git --version 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-CheckResult -Item "Git" -Status Pass -Detail $gitOutput.Trim()
    } else {
        throw "Git 命令执行失败"
    }
} catch [System.Management.Automation.CommandNotFoundException] {
    Write-CheckResult -Item "Git" -Status Warn -Detail "未检测到 Git" `
        -Suggestion "Git 不是必需项，但建议安装以便更新 NOVA。可从 https://git-scm.com 下载。"
} catch {
    Write-CheckResult -Item "Git" -Status Warn -Detail "检测异常: $_" `
        -Suggestion "Git 不是必需项，但建议重新安装以方便后续更新。"
}

# ============================================
# 6. Claude CLI
# ============================================
try {
    $claudeOutput = & claude --version 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-CheckResult -Item "Claude CLI" -Status Pass -Detail $claudeOutput.Trim()
    } else {
        throw "Claude CLI 命令执行失败"
    }
} catch [System.Management.Automation.CommandNotFoundException] {
    Write-CheckResult -Item "Claude CLI" -Status Warn -Detail "未安装" `
        -Suggestion "Claude CLI 不是必需项，如您使用 Claude Code 则需安装。请参考 Anthropic 官方文档。"
} catch {
    Write-CheckResult -Item "Claude CLI" -Status Warn -Detail "检测异常: $_" `
        -Suggestion "Claude CLI 不是必需项，但建议检查安装状态。"
}

# ============================================
# 7. 磁盘剩余空间
# ============================================
try {
    $systemDrive = [System.IO.Path]::GetPathRoot($env:SystemRoot)
    $drive = Get-CimInstance Win32_LogicalDisk -Filter "DeviceID='$systemDrive'" -ErrorAction Stop
    $freeMB = [math]::Round($drive.FreeSpace / 1MB)
    $freeGB = [math]::Round($drive.FreeSpace / 1GB, 1)

    if ($freeMB -ge $RequiredDiskSpaceMB) {
        Write-CheckResult -Item "磁盘剩余空间 ($systemDrive)" -Status Pass -Detail "${freeGB} GB"
    } elseif ($freeMB -ge $MinDiskSpaceMB) {
        Write-CheckResult -Item "磁盘剩余空间 ($systemDrive)" -Status Warn -Detail "${freeGB} GB（建议: 2 GB 以上）" `
            -Suggestion "磁盘空间偏低，建议清理磁盘后再安装 NOVA。"
    } else {
        Write-CheckResult -Item "磁盘剩余空间 ($systemDrive)" -Status Fail -Detail "${freeGB} GB（最低: 1 GB）" `
            -Suggestion "磁盘空间不足，请先释放至少 2 GB 可用空间。"
    }
} catch {
    Write-CheckResult -Item "磁盘剩余空间" -Status Warn -Detail "无法获取磁盘信息: $_" `
        -Suggestion "请手动确认系统盘有至少 2 GB 剩余空间。"
}

# ============================================
# 8. PATH 环境变量检查
# ============================================
try {
    $pathEntries = $env:PATH -split ';' | Where-Object { $_ -ne '' }
    $suspicious = @()

    foreach ($entry in $pathEntries) {
        $trimmed = $entry.TrimEnd('\')
        # 检查是否为空路径或只有空格
        if ([string]::IsNullOrWhiteSpace($trimmed)) {
            if ($suspicious -notcontains "(空路径)") {
                $suspicious += "(空路径)"
            }
            continue
        }
        # 检查路径是否存在
        if (-not (Test-Path $trimmed -ErrorAction SilentlyContinue)) {
            $suspicious += $trimmed
        }
        # 检查路径中是否有不可打印字符（常见乱码）
        if ($trimmed -match '[^\x20-\x7E一-鿿＀-￯\\:._\-\(\)\s]') {
            if ($suspicious -notcontains "(含异常字符) $trimmed") {
                $suspicious += "(含异常字符) $trimmed"
            }
        }
    }

    $totalCount = ($pathEntries | Measure-Object).Count

    if ($suspicious.Count -eq 0) {
        Write-CheckResult -Item "PATH 环境变量" -Status Pass -Detail "共 $totalCount 条，未发现异常"
    } else {
        $sb = New-Object System.Text.StringBuilder
        $null = $sb.AppendLine("共 $totalCount 条，其中 $($suspicious.Count) 条异常:")
        foreach ($s in $suspicious) {
            $null = $sb.AppendLine("                     - $s")
        }
        Write-CheckResult -Item "PATH 环境变量" -Status Warn -Detail $sb.ToString().TrimEnd() `
            -Suggestion "部分 PATH 条目指向不存在的路径或含异常字符。建议清理系统环境变量。"
    }
} catch {
    Write-CheckResult -Item "PATH 环境变量" -Status Warn -Detail "无法检测: $_"
}

# ============================================
# 9. 系统内存
# ============================================
try {
    $os = Get-CimInstance Win32_OperatingSystem -ErrorAction Stop
    $totalMemGB = [math]::Round($os.TotalVisibleMemorySize / 1MB, 1)
    $freeMemGB = [math]::Round($os.FreePhysicalMemory / 1MB, 1)
    $usedPercent = [math]::Round(($os.TotalVisibleMemorySize - $os.FreePhysicalMemory) / $os.TotalVisibleMemorySize * 100)

    if ($freeMemGB -ge 4) {
        Write-CheckResult -Item "系统内存" -Status Pass -Detail "总计 ${totalMemGB} GB，可用 ${freeMemGB} GB (${usedPercent}% 已用)"
    } elseif ($freeMemGB -ge 2) {
        Write-CheckResult -Item "系统内存" -Status Warn -Detail "总计 ${totalMemGB} GB，可用 ${freeMemGB} GB (${usedPercent}% 已用)" `
            -Suggestion "可用内存偏低，NOVA 运行可能受影响。建议关闭其他程序后再运行。"
    } else {
        Write-CheckResult -Item "系统内存" -Status Fail -Detail "总计 ${totalMemGB} GB，可用 ${freeMemGB} GB (${usedPercent}% 已用)" `
            -Suggestion "可用内存严重不足，请关闭不必要的程序后重试。"
    }
} catch {
    Write-CheckResult -Item "系统内存" -Status Warn -Detail "无法获取内存信息: $_"
}

# ============================================
# 10. 杀毒/安全软件检测
# ============================================
try {
    $avProcesses = @(
        @{ Name="360安全卫士";   Pattern="360tray|360sd|zhudongfangyu" },
        @{ Name="火绒安全";     Pattern="HipsTray|HipsDaemon|wsctrl" },
        @{ Name="腾讯电脑管家"; Pattern="QQPCTray|QQPCRtp|QQPCMgr" },
        @{ Name="Windows Defender"; Pattern="MsMpEng|SecurityHealthService" }
    )

    $found = @()
    foreach ($av in $avProcesses) {
        $proc = Get-Process -Name $av.Pattern.Split('|') -ErrorAction SilentlyContinue
        if ($proc) {
            $found += $av.Name
        }
    }

    if ($found.Count -eq 0) {
        Write-CheckResult -Item "安全软件" -Status Pass -Detail "未检测到第三方杀软运行"
    } else {
        $names = $found -join "、"
        Write-CheckResult -Item "安全软件" -Status Warn -Detail "检测到: $names" `
            -Suggestion "部分安全软件可能拦截 NOVA 的网络请求或文件操作。如遇到连接/安装问题，请尝试将 NOVA 加入白名单。"
    }
} catch {
    Write-CheckResult -Item "安全软件" -Status Warn -Detail "无法检测安全软件: $_"
}

# ============================================
# 11. 网络代理设置
# ============================================
try {
    $proxyVars = @()
    if ($env:HTTP_PROXY)     { $proxyVars += "HTTP_PROXY=$($env:HTTP_PROXY)" }
    if ($env:HTTPS_PROXY)    { $proxyVars += "HTTPS_PROXY=$($env:HTTPS_PROXY)" }
    if ($env:http_proxy)     { $proxyVars += "http_proxy=$($env:http_proxy)" }
    if ($env:https_proxy)    { $proxyVars += "https_proxy=$($env:https_proxy)" }
    if ($env:NO_PROXY)       { $proxyVars += "NO_PROXY=$($env:NO_PROXY)" }
    if ($env:ALL_PROXY)      { $proxyVars += "ALL_PROXY=$($env:ALL_PROXY)" }

    # 也检查系统代理设置
    $sysProxy = (Get-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings" -ErrorAction SilentlyContinue)
    $proxyEnabled = if ($sysProxy.ProxyEnable -eq 1) { "系统代理已启用" } else { "" }

    if ($proxyVars.Count -eq 0 -and -not $proxyEnabled) {
        Write-CheckResult -Item "网络代理" -Status Pass -Detail "未配置代理（环境变量与系统代理均未启用）"
    } elseif ($proxyVars.Count -gt 0) {
        $detail = ($proxyVars -join " | ")
        if ($proxyEnabled) { $detail += " | $proxyEnabled" }
        Write-CheckResult -Item "网络代理" -Status Warn -Detail $detail `
            -Suggestion "检测到代理配置。如 NOVA 无法连接网络，请检查代理设置是否正确。"
    } elseif ($proxyEnabled) {
        Write-CheckResult -Item "网络代理" -Status Warn -Detail "$proxyEnabled" `
            -Suggestion "系统代理已启用，如遇到网络问题请检查代理设置。"
    }
} catch {
    Write-CheckResult -Item "网络代理" -Status Warn -Detail "无法检测代理设置: $_"
}

# ============================================
# 12. Tauri 日志文件
# ============================================
try {
    # Tauri 2 日志默认存放在 %APPDATA%\com.nova.app\logs 或 %LOCALAPPDATA%\com.nova.app
    $tauriLogPaths = @(
        "$env:APPDATA\com.nova.app",
        "$env:LOCALAPPDATA\com.nova.app"
    )

    $logFound = $false
    $logDetail = ""
    foreach ($p in $tauriLogPaths) {
        if (Test-Path $p) {
            $logFound = $true
            # 找 .log 文件
            $logFiles = Get-ChildItem -Path $p -Recurse -Filter "*.log" -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending
            if ($logFiles -and $logFiles.Count -gt 0) {
                $latest = $logFiles[0]
                $sizeKB = [math]::Round($latest.Length / 1KB, 1)
                $logDetail = "最新日志: $($latest.Name) (${sizeKB} KB, $($latest.LastWriteTime.ToString('yyyy-MM-dd HH:mm'))), 共 $($logFiles.Count) 个日志文件"
                break
            } else {
                $logDetail = "数据目录存在但无日志文件"
                break
            }
        }
    }

    if ($logFound) {
        Write-CheckResult -Item "Tauri 日志" -Status Pass -Detail $logDetail
    } else {
        Write-CheckResult -Item "Tauri 日志" -Status Pass -Detail "未发现历史日志（首次运行或已清理）"
    }
} catch {
    Write-CheckResult -Item "Tauri 日志" -Status Warn -Detail "无法检查日志文件: $_"
}

# ============================================
# 13. WebView2 用户数据目录
# ============================================
try {
    $wv2DataPaths = @(
        "$env:LOCALAPPDATA\com.nova.claude-code",
        "$env:APPDATA\com.nova.app",
        "$env:LOCALAPPDATA\Microsoft\Edge\User Data"
    )

    $wv2DataExists = $false
    $wv2DataDetail = ""
    foreach ($p in $wv2DataPaths) {
        if (Test-Path $p) {
            $wv2DataExists = $true
            try {
                $size = [math]::Round((Get-ChildItem -Path $p -Recurse -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum / 1MB, 1)
                if ($size -gt 0) {
                    $wv2DataDetail += "$([System.IO.Path]::GetFileName($p)): ${size} MB; "
                }
            } catch {}
        }
    }

    if ($wv2DataExists -and $wv2DataDetail) {
        Write-CheckResult -Item "WebView2 数据目录" -Status Pass -Detail $wv2DataDetail.TrimEnd('; ')
    } elseif ($wv2DataExists) {
        Write-CheckResult -Item "WebView2 数据目录" -Status Pass -Detail "数据目录存在（大小忽略不计）"
    } else {
        Write-CheckResult -Item "WebView2 数据目录" -Status Pass -Detail "未发现数据目录（首次运行或已清理）"
    }
} catch {
    Write-CheckResult -Item "WebView2 数据目录" -Status Warn -Detail "无法检查: $_"
}

# ============================================
# 14. Windows 事件日志检查
# ============================================
try {
    # 检查最近是否有应用崩溃/错误事件（过去 7 天）
    $since = (Get-Date).AddDays(-7)
    $crashEvents = Get-WinEvent -LogName Application -MaxEvents 5 -ErrorAction SilentlyContinue |
        Where-Object { $_.TimeCreated -ge $since -and $_.LevelDisplayName -in @("错误", "Error") } |
        Select-Object -First 3

    if ($crashEvents -and $crashEvents.Count -gt 0) {
        $detailList = @()
        foreach ($evt in $crashEvents) {
            $detailList += "事件ID $($evt.Id) ($($evt.ProviderName)): $($evt.TimeCreated.ToString('MM-dd HH:mm'))"
        }
        Write-CheckResult -Item "系统事件日志" -Status Warn -Detail "最近 7 天有 $($crashEvents.Count)+ 条错误事件:`n$($detailList -join "`n")" `
            -Suggestion "部分系统错误可能影响 NOVA 运行。如果 NOVA 出现异常，可查看事件查看器获得更多线索。"
    } else {
        Write-CheckResult -Item "系统事件日志" -Status Pass -Detail "最近 7 天未发现明显系统错误"
    }
} catch [System.Exception] {
    if ($_.Exception.Message -match "No events were found") {
        Write-CheckResult -Item "系统事件日志" -Status Pass -Detail "应用程序日志无错误记录"
    } else {
        Write-CheckResult -Item "系统事件日志" -Status Warn -Detail "无法读取事件日志: $_" `
            -Suggestion "事件日志读取失败通常不意味着系统问题，可忽略。"
    }
}

# ============================================
# 15. 联网能力检查
# ============================================
try {
    $pingResult = Test-Connection -ComputerName "registry.npmjs.org" -Count 1 -Quiet -ErrorAction SilentlyContinue
    if ($pingResult) {
        Write-CheckResult -Item "网络连接" -Status Pass -Detail "可访问外部网络（npm registry）"
    } else {
        Write-CheckResult -Item "网络连接" -Status Warn -Detail "无法连接到 npm registry" `
            -Suggestion "如果处于离线环境请忽略。在线环境请检查网络设置或代理配置。"
    }
} catch {
    Write-CheckResult -Item "网络连接" -Status Warn -Detail "无法检测网络状态" `
        -Suggestion "如果处于离线环境请忽略。"
}

# ============================================
# 底部总结
# ============================================
Write-Host "============================================" -ForegroundColor Cyan
Write-Host "  诊断完成。" -ForegroundColor Cyan
Write-Host ""
Write-Host "  图例:  ✅ 通过  ⚠️ 需注意  ❌ 失败" -ForegroundColor Gray
Write-Host "============================================" -ForegroundColor Cyan
Write-Host ""

# 暂停让用户看到结果
Write-Host "按任意键退出..." -ForegroundColor Gray
$null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
