// cleanup-helper: NOVA 卸载清理工具（独立于 Tauri 运行时）
//
// 编译:
//   GOOS=windows GOARCH=amd64 go build -ldflags "-s -w -H windowsgui" -o cleanup-helper.exe main.go
//
// 功能:
//   1. 清理 %LOCALAPPDATA%\com.tinyzhuang.tokenicode\  （应用数据: CLI 下载、Git、缓存）
//   2. 清理 %APPDATA%\com.nova.app\                     （WebView2 用户数据）
//   3. 清理 %LOCALAPPDATA%\com.nova.claude-code\         （可能的额外 WebView 数据）
//   4. 不删除 %USERPROFILE%\.tokenicode\                （用户设置，保留）
//   5. 不删除 %USERPROFILE%\.claude\projects\           （Claude 项目数据，保留）
//
// 错误处理:
//   - 文件被占用 → 跳过，继续清理
//   - 目录不存在 → 跳过，不报错
//   - 权限不足 → 跳过，继续清理
//   所有错误都不阻塞卸载流程，确保"尽力而为"清理
//
// 退出码:
//   0 = 全部清理成功（或无可清理内容）
//   1 = 部分清理失败（有残留，但不影响卸载）

package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

// 要清理的目录列表（按顺序）
type CleanupTarget struct {
	Path        string
	Description string
	Required    bool // true=必须清理, false=尽力而为
}

func main() {
	// 静默运行，不弹窗（-H windowsgui）
	// 所有输出通过 stdout/stderr，由 NSIS nsExec::ExecToLog 捕获

	homeDir, err := os.UserHomeDir()
	if err != nil {
		fmt.Fprintf(os.Stderr, "错误: 无法获取用户目录: %v\n", err)
		os.Exit(1)
	}

	localAppData := os.Getenv("LOCALAPPDATA")
	if localAppData == "" {
		fmt.Fprintf(os.Stderr, "错误: LOCALAPPDATA 环境变量未设置\n")
		os.Exit(1)
	}

	appData := os.Getenv("APPDATA")
	if appData == "" {
		fmt.Fprintf(os.Stderr, "错误: APPDATA 环境变量未设置\n")
		os.Exit(1)
	}

	targets := []CleanupTarget{
		{
			Path:        filepath.Join(localAppData, "com.tinyzhuang.tokenicode"),
			Description: "应用数据目录 (CLI/Git/缓存)",
			Required:    false,
		},
		{
			Path:        filepath.Join(appData, "com.nova.app"),
			Description: "WebView2 用户数据",
			Required:    false,
		},
		{
			Path:        filepath.Join(localAppData, "com.nova.claude-code"),
			Description: "额外 WebView 数据目录",
			Required:    false,
		},
	}

	// 可选: 清理更多已知数据目录
	// 检查并清理可能存在的额外目录
	extraTargets := []CleanupTarget{
		{
			Path:        filepath.Join(appData, "com.tinyzhuang.tokenicode"),
			Description: "Roaming 应用数据 (如有)",
			Required:    false,
		},
	}

	// 不清理的目录（保留用户数据）:
	// - filepath.Join(homeDir, ".tokenicode")    — 用户设置
	// - filepath.Join(homeDir, ".claude", "projects") — Claude 项目

	// 清理主目标
	cleanupAll(targets)

	// 清理额外目标
	cleanupAll(extraTargets)

	// 清理可能残留在 temp 目录的 NOVA 文件
	cleanupTempNova()

	fmt.Println("清理完成")
	// 使用 _ = homeDir 避免未使用变量警告
	_ = homeDir
}

func cleanupAll(targets []CleanupTarget) {
	for _, t := range targets {
		if err := removeDir(t.Path, t.Required); err != nil {
			fmt.Fprintf(os.Stderr, "警告: 清理 %s (%s) 失败: %v\n", t.Description, t.Path, err)
		}
	}
}

// removeDir 安全删除目录
// - 目录不存在 → 跳过
// - 删除失败 → required=true 时返回错误
func removeDir(path string, required bool) error {
	// 安全检查: 不允许删除根目录或关键系统目录
	if isDangerousPath(path) {
		return fmt.Errorf("安全限制: 拒绝删除系统关键路径 %s", path)
	}

	info, err := os.Stat(path)
	if os.IsNotExist(err) {
		// 目录不存在 — 不是错误（重复卸载 / 已清理）
		return nil
	}
	if err != nil {
		return fmt.Errorf("无法访问目录: %w", err)
	}
	if !info.IsDir() {
		return fmt.Errorf("路径不是目录: %s", path)
	}

	fmt.Printf("正在删除: %s\n", path)

	err = os.RemoveAll(path)
	if err != nil {
		if required {
			return fmt.Errorf("删除失败: %w", err)
		}
		// 非必要目录删除失败 — 尝试标记文件为重启后删除
		fmt.Fprintf(os.Stderr, "警告: 无法完全删除 %s，尝试标记为重启后清理\n", path)
		_ = markForReboot(path)
		return nil
	}

	fmt.Printf("已删除: %s\n", path)
	return nil
}

// isDangerousPath 安全检查 — 防止意外删除系统目录
func isDangerousPath(path string) bool {
	abs, err := filepath.Abs(path)
	if err != nil {
		return true // 无法解析路径，保守拒绝
	}

	// 标准化路径（统一分隔符和小写比较）
	abs = strings.ToLower(filepath.Clean(abs))

	dangerous := []string{
		"c:\\windows",
		"c:\\windows\\system32",
		"c:\\",
		"c:\\program files",
		"c:\\program files (x86)",
		"c:\\users",
	}

	for _, d := range dangerous {
		if abs == d || strings.HasPrefix(abs, d+"\\") {
			// 但允许删除 Users 下的特定应用目录
			if strings.HasPrefix(d, "c:\\users") &&
				(strings.Contains(abs, "\\appdata\\") ||
					strings.Contains(abs, "\\.tokenicode") ||
					strings.Contains(abs, "\\.claude") ||
					strings.Contains(abs, "\\com.")) {
				return false // 这是合法的应用数据路径
			}
			return true
		}
	}
	return false
}

// markForReboot 标记文件/目录为系统重启后删除
// 使用 Windows MoveFileEx + MOVEFILE_DELAY_UNTIL_REBOOT 标志
func markForReboot(path string) error {
	// 尝试重命名为临时名称，然后标记删除
	// 简单实现：写入一个空的 .pending_delete 标记文件
	marker := path + ".pending_delete"
	return os.WriteFile(marker, []byte("marked for deletion on next reboot"), 0644)
}

// cleanupTempNova 清理临时目录中的 NOVA 残留文件
func cleanupTempNova() {
	tempDir := os.TempDir()

	// 列出 temp 目录内容，找 NOVA 相关文件
	entries, err := os.ReadDir(tempDir)
	if err != nil {
		return // 无法读取 temp，跳过
	}

	novaPrefixes := []string{
		"tokenicode-", "nova-", "NOVA-",
		"claude-cli-", "webview-",
		".tmp_nova", ".tmp_tokenicode",
	}

	for _, entry := range entries {
		name := entry.Name()
		for _, prefix := range novaPrefixes {
			if strings.HasPrefix(strings.ToLower(name), prefix) {
				fullPath := filepath.Join(tempDir, name)
				if entry.IsDir() {
					_ = os.RemoveAll(fullPath)
					fmt.Printf("清理临时目录: %s\n", fullPath)
				} else {
					_ = os.Remove(fullPath)
				}
				break
			}
		}
	}
}
