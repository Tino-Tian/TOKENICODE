//! 首次启动自动部署 Claude Code CLI。
//!
//! Bug 1 (P0): 安装后需手动下载 Claude Code CLI
//! 目标：首次启动自动检测 → 不存在则静默下载部署 → 写入 PATH
//!
//! 流程：
//!   1. 调用 Tier 0-4 多级检测（cli_resolver::resolve）
//!   2. 全部未找到 → 检查磁盘空间 ≥ 200MB
//!   3. 空间足够 → 调用 native_download 下载到 %LOCALAPPDATA%/NOVA/cli/
//!   4. 支持断点续传（HTTP Range）
//!   5. 下载成功 → 写入 HKCU\Environment\Path（复用 finalize_cli_install_paths）
//!   6. 失败 → 返回明确错误类型（权限/网络/磁盘/杀软）
//!   7. 连续失败 3 次 → 提示手动下载

use serde::Serialize;
use std::path::{Path, PathBuf};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

// ─── 错误分类 ──────────────────────────────────────────────────

/// 下载失败的具体原因
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum DeployErrorKind {
    /// 磁盘空间不足（< 200MB）
    DiskSpaceLow,
    /// 网络不通/超时
    NetworkError,
    /// 文件写入权限不足
    PermissionDenied,
    /// 可能是杀软拦截（文件写入后立即消失或无法重命名）
    AntivirusBlock,
    /// 所有下载源都失败
    AllSourcesFailed,
    /// 其他未知错误
    Unknown,
}

impl std::fmt::Display for DeployErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::DiskSpaceLow => write!(f, "磁盘空间不足"),
            Self::NetworkError => write!(f, "网络连接失败"),
            Self::PermissionDenied => write!(f, "文件写入权限不足"),
            Self::AntivirusBlock => write!(f, "可能被杀毒软件拦截"),
            Self::AllSourcesFailed => write!(f, "所有下载源均不可达"),
            Self::Unknown => write!(f, "未知错误"),
        }
    }
}

/// 部署结果返回给前端
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployResult {
    /// CLI 找到或下载成功
    pub success: bool,
    /// CLI 路径
    pub cli_path: Option<String>,
    /// CLI 版本
    pub version: Option<String>,
    /// 失败时的错误分类
    pub error_kind: Option<DeployErrorKind>,
    /// 人类可读的错误消息
    pub error_message: Option<String>,
    /// 是否已失败 3 次以上（应展示手动下载指引）
    pub suggest_manual_download: bool,
    /// 手动下载 URL
    pub manual_download_url: Option<String>,
}

// ─── 失败计数持久化 ───────────────────────────────────────────

/// 持久化失败计数文件路径：~/.her/deploy-fail-count.json
fn deploy_fail_count_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".her").join("deploy-fail-count.json"))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct DeployFailCount {
    count: u32,
    last_error: Option<String>,
    last_error_kind: Option<String>,
}

fn read_fail_count() -> DeployFailCount {
    let path = match deploy_fail_count_path() {
        Some(p) => p,
        None => return DeployFailCount { count: 0, last_error: None, last_error_kind: None },
    };
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or(DeployFailCount {
            count: 0,
            last_error: None,
            last_error_kind: None,
        }),
        Err(_) => DeployFailCount { count: 0, last_error: None, last_error_kind: None },
    }
}

fn write_fail_count(fc: &DeployFailCount) {
    if let Some(path) = deploy_fail_count_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string(fc) {
            let _ = std::fs::write(&path, json);
        }
    }
}

fn increment_fail_count(error: &str, kind: Option<&DeployErrorKind>) -> u32 {
    let mut fc = read_fail_count();
    fc.count += 1;
    fc.last_error = Some(error.to_string());
    fc.last_error_kind = kind.map(|k| format!("{:?}", k));
    write_fail_count(&fc);
    fc.count
}

fn reset_fail_count() {
    write_fail_count(&DeployFailCount {
        count: 0,
        last_error: None,
        last_error_kind: None,
    });
}

// ─── 磁盘空间检查 ─────────────────────────────────────────────

/// 检查 CLI 安装目录所在磁盘剩余空间是否 >= 200MB。
/// Windows: 检查 %LOCALAPPDATA% 所在驱动器
/// macOS/Linux: 检查 ~/.claude/local/ 所在分区
fn check_disk_space(install_dir: &Path) -> Result<u64, String> {
    // 确保目录存在后才能检查
    if let Some(parent) = install_dir.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    #[cfg(target_os = "windows")]
    {
        // Windows 用 GetDiskFreeSpaceEx 等效：检查安装目录所在盘
        let dir_str = install_dir.to_string_lossy().to_string();
        // 提取盘符
        let drive = if dir_str.len() >= 2 && dir_str.as_bytes()[1] == b':' {
            &dir_str[..2]
        } else {
            "C:"
        };
        // 用 PowerShell 查剩余空间（WMI 路径）
        let ps = format!(
            "(Get-WmiObject Win32_LogicalDisk -Filter \"DeviceID='{}'\").FreeSpace",
            drive
        );
        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &ps])
            .creation_flags(0x08000000)
            .output();
        match output {
            Ok(o) if o.status.success() => {
                let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                match s.parse::<u64>() {
                    Ok(bytes) => return Ok(bytes),
                    Err(_) => {
                        // 降级：尝试用 std::fs 检查（不准确但兜底）
                        eprintln!("[cli_deploy] WMI query failed, skip disk check");
                        return Ok(u64::MAX); // 无法获取则不阻塞
                    }
                }
            }
            _ => {
                eprintln!("[cli_deploy] PowerShell disk check failed, skip");
                return Ok(u64::MAX);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        // macOS/Linux: df 命令
        let dir_str = install_dir.to_string_lossy().to_string();
        let output = std::process::Command::new("df")
            .args(["-k", &dir_str])
            .output();
        match output {
            Ok(o) if o.status.success() => {
                let stdout = String::from_utf8_lossy(&o.stdout);
                // df -k 输出第二行最后一列是可用 KB
                if let Some(line) = stdout.lines().nth(1) {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 4 {
                        if let Ok(kb) = parts[3].parse::<u64>() {
                            return Ok(kb * 1024); // 转换为字节
                        }
                    }
                }
                eprintln!("[cli_deploy] df parse failed, skip disk check");
                return Ok(u64::MAX);
            }
            _ => {
                eprintln!("[cli_deploy] df failed, skip disk check");
                return Ok(u64::MAX);
            }
        }
    }
}

const MIN_FREE_SPACE: u64 = 200 * 1024 * 1024; // 200MB

// ─── 错误分类 ─────────────────────────────────────────────────

/// 将下载/安装过程中遇到的错误分类为明确的 DeployErrorKind。
pub fn classify_error(error_str: &str) -> DeployErrorKind {
    let lower = error_str.to_lowercase();

    // 磁盘空间
    if lower.contains("no space")
        || lower.contains("disk full")
        || lower.contains("not enough space")
        || lower.contains("enospc")
        || lower.contains("磁盘空间")
    {
        return DeployErrorKind::DiskSpaceLow;
    }

    // 权限
    if lower.contains("permission denied")
        || lower.contains("access denied")
        || lower.contains("access is denied")
        || lower.contains("eperm")
        || lower.contains("eacces")
        || lower.contains("operation not permitted")
        || lower.contains("无法访问")
    {
        return DeployErrorKind::PermissionDenied;
    }

    // 网络
    if lower.contains("timeout")
        || lower.contains("timed out")
        || lower.contains("network")
        || lower.contains("connect")
        || lower.contains("enotfound")
        || lower.contains("econnrefused")
        || lower.contains("econnreset")
        || lower.contains("etimedout")
        || lower.contains("dns")
        || lower.contains("certificate")
        || lower.contains("resolve")
        || lower.contains("无法连接")
        || lower.contains("couldn't connect")
    {
        return DeployErrorKind::NetworkError;
    }

    // 杀软（文件写入后立即消失或拒绝访问）
    if lower.contains("antivirus")
        || lower.contains("virus")
        || lower.contains("defender")
        || lower.contains("blocked")
        || lower.contains("blocked by")
        || lower.contains("windows defender")
        || lower.contains("被拦截")
        || lower.contains("杀毒")
    {
        return DeployErrorKind::AntivirusBlock;
    }

    // 所有源都失败
    if lower.contains("all native download sources")
        || lower.contains("all install methods failed")
        || lower.contains("all sources")
    {
        return DeployErrorKind::AllSourcesFailed;
    }

    DeployErrorKind::Unknown
}

// ─── 手动下载指引 ─────────────────────────────────────────────

/// 获取 Claude Code CLI 手动下载地址
pub fn manual_download_url() -> &'static str {
    "https://docs.anthropic.com/en/docs/claude-code/overview"
}

// ─── 首次启动自动部署逻辑 ─────────────────────────────────────

/// 首次启动时自动检测并部署 CLI。
///
/// 这个函数供 `auto_deploy_cli` Tauri 命令调用。
/// 返回 `DeployResult` 供前端决定下一步。
pub async fn perform_auto_deploy(
    app: Option<&tauri::AppHandle>,
    force_install: bool,
) -> DeployResult {
    eprintln!("[cli_deploy] auto deploy start, force_install={force_install}");

    // Step 1: 先运行 Tier 0-4 多级检测
    if !force_install {
        if let Some((path, source)) = super::cli_resolver::resolve() {
            eprintln!("[cli_deploy] CLI found at {path} (source: {source})");
            reset_fail_count();

            // 已经找到，确保 PATH 中有它
            // 这里不阻塞主流程，PATH 注入放在 finalize_cli_install_paths 中处理
            return DeployResult {
                success: true,
                cli_path: Some(path),
                version: None, // 版本检查留给前端异步做
                error_kind: None,
                error_message: None,
                suggest_manual_download: false,
                manual_download_url: None,
            };
        }
    }

    eprintln!("[cli_deploy] CLI not found, starting download...");

    // Step 2: 检查磁盘空间
    let install_dir = crate::cli_download_dir().unwrap_or_else(|| {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("NOVA")
            .join("cli")
    });

    match check_disk_space(&install_dir) {
        Ok(free_bytes) => {
            if free_bytes < MIN_FREE_SPACE {
                let msg = format!(
                    "磁盘空间不足: 需要至少 200MB，当前可用 {}MB",
                    free_bytes / (1024 * 1024)
                );
                eprintln!("[cli_deploy] {}", msg);
                increment_fail_count(&msg, Some(&DeployErrorKind::DiskSpaceLow));
                return DeployResult {
                    success: false,
                    cli_path: None,
                    version: None,
                    error_kind: Some(DeployErrorKind::DiskSpaceLow),
                    error_message: Some(msg),
                    suggest_manual_download: false,
                    manual_download_url: Some(manual_download_url().to_string()),
                };
            }
        }
        Err(e) => {
            eprintln!("[cli_deploy] disk check failed: {e}, continuing anyway");
        }
    }

    // Step 3: 尝试下载（复用现有 install_claude_cli 或 try_native_cli_download）
    // 注意：这里的逻辑是直接调用现有下载能力，新增断点续传在 try_native_cli_download 中处理
    if let Some(app_handle) = app {
        match install_cli_silent(app_handle).await {
            Ok((path, version)) => {
                eprintln!("[cli_deploy] download success: {path}");
                reset_fail_count();
                return DeployResult {
                    success: true,
                    cli_path: Some(path),
                    version: Some(version),
                    error_kind: None,
                    error_message: None,
                    suggest_manual_download: false,
                    manual_download_url: None,
                };
            }
            Err(e) => {
                let kind = classify_error(&e);
                let new_count = increment_fail_count(&e, Some(&kind));
                let suggest_manual = new_count >= 3;

                eprintln!(
                    "[cli_deploy] download failed (attempt {}): {} (kind: {:?})",
                    new_count, e, kind
                );

                return DeployResult {
                    success: false,
                    cli_path: None,
                    version: None,
                    error_kind: Some(kind),
                    error_message: Some(e),
                    suggest_manual_download: suggest_manual,
                    manual_download_url: if suggest_manual {
                        Some(manual_download_url().to_string())
                    } else {
                        None
                    },
                };
            }
        }
    }

    // 没有 AppHandle（罕见情况）
    DeployResult {
        success: false,
        cli_path: None,
        version: None,
        error_kind: Some(DeployErrorKind::Unknown),
        error_message: Some("AppHandle not available".to_string()),
        suggest_manual_download: false,
        manual_download_url: None,
    }
}

/// 静默安装 CLI（不弹出额外 UI，通过 AppHandle 发进度事件）。
async fn install_cli_silent(app: &tauri::AppHandle) -> Result<(String, String), String> {
    // 直接复用 try_native_cli_download 的核心逻辑
    // 但使用 cli_download_dir() 而非 ~/.claude/local/

    let china = crate::is_china_network().await;
    let version = crate::try_native_cli_download(Some(app), china).await?;

    // 下载成功 → 写入 PATH
    crate::finalize_cli_install_paths(app);

    // 获取安装路径
    let cli_path = super::cli_resolver::resolve()
        .map(|(p, _)| p)
        .unwrap_or_else(|| {
            crate::cli_download_dir()
                .map(|d| {
                    d.join(if cfg!(target_os = "windows") {
                        "claude.exe"
                    } else {
                        "claude"
                    })
                    .to_string_lossy()
                    .to_string()
                })
                .unwrap_or_default()
        });

    Ok((cli_path, version))
}

// ─── Tauri 命令 ──────────────────────────────────────────────

/// Tauri 命令：首次启动自动检测并部署 CLI。
///
/// 前端在 SetupWizard mount 时调用此命令。
/// 如果 CLI 已存在 → 直接返回成功。
/// 如果 CLI 不存在 → 自动下载部署。
///
/// `force_install`: 如果为 true，即使已有 CLI 也重新下载（用于重试按钮）。
#[tauri::command]
pub async fn auto_deploy_cli(
    app: tauri::AppHandle,
    force_install: bool,
) -> Result<DeployResult, String> {
    Ok(perform_auto_deploy(Some(&app), force_install).await)
}

/// 查询当前失败计数（供前端使用）
#[tauri::command]
pub fn get_deploy_fail_count() -> Result<u32, String> {
    let fc = read_fail_count();
    Ok(fc.count)
}

/// 重置失败计数（用户手动触发）
#[tauri::command]
pub fn reset_deploy_fail_count() -> Result<(), String> {
    reset_fail_count();
    Ok(())
}

// ─── 测试 ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_error_disk_space() {
        assert_eq!(
            classify_error("No space left on device"),
            DeployErrorKind::DiskSpaceLow
        );
        assert_eq!(
            classify_error("ENOSPC: disk full"),
            DeployErrorKind::DiskSpaceLow
        );
        assert_eq!(
            classify_error("磁盘空间不足"),
            DeployErrorKind::DiskSpaceLow
        );
    }

    #[test]
    fn test_classify_error_permission() {
        assert_eq!(
            classify_error("Permission denied (os error 13)"),
            DeployErrorKind::PermissionDenied
        );
        assert_eq!(
            classify_error("Access is denied"),
            DeployErrorKind::PermissionDenied
        );
        assert_eq!(
            classify_error("EACCES"),
            DeployErrorKind::PermissionDenied
        );
    }

    #[test]
    fn test_classify_error_network() {
        assert_eq!(
            classify_error("Connection timed out"),
            DeployErrorKind::NetworkError
        );
        assert_eq!(
            classify_error("ETIMEDOUT"),
            DeployErrorKind::NetworkError
        );
        assert_eq!(
            classify_error("Couldn't connect to server"),
            DeployErrorKind::NetworkError
        );
    }

    #[test]
    fn test_classify_error_antivirus() {
        assert_eq!(
            classify_error("Operation blocked by Windows Defender"),
            DeployErrorKind::AntivirusBlock
        );
        assert_eq!(
            classify_error("文件被杀毒软件拦截"),
            DeployErrorKind::AntivirusBlock
        );
    }

    #[test]
    fn test_classify_error_all_sources() {
        assert_eq!(
            classify_error("All native download sources failed"),
            DeployErrorKind::AllSourcesFailed
        );
    }

    #[test]
    fn test_classify_error_unknown() {
        assert_eq!(
            classify_error("Some random error"),
            DeployErrorKind::Unknown
        );
    }

    #[test]
    fn test_deploy_result_serialization() {
        let r = DeployResult {
            success: true,
            cli_path: Some("/usr/local/bin/claude".to_string()),
            version: Some("1.0.0".to_string()),
            error_kind: None,
            error_message: None,
            suggest_manual_download: false,
            manual_download_url: None,
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("success"));
        assert!(json.contains("cliPath"));
    }
}
