import { useEffect, useCallback, useState, useRef } from 'react';
import { useSetupStore } from '../../stores/setupStore';
import { useSettingsStore } from '../../stores/settingsStore';
import { useProviderStore } from '../../stores/providerStore';
import { useT } from '../../lib/i18n';
import { stripAnsi } from '../../lib/strip-ansi';
import { AiAvatar } from '../shared/AiAvatar';
import {
  bridge,
  onDownloadProgress,
} from '../../lib/tauri-bridge';
import type { DeployResult } from '../../lib/tauri-bridge';

/** 将 Rust DeployResult 的 errorKind 映射为本地错误提示函数 */
function isPermissionError(kind: string | null | undefined): boolean {
  return kind === 'PermissionDenied';
}

function isNetworkError(kind: string | null | undefined): boolean {
  return kind === 'NetworkError' || kind === 'AllSourcesFailed';
}

function isDiskError(kind: string | null | undefined): boolean {
  return kind === 'DiskSpaceLow';
}

function isAntivirusError(kind: string | null | undefined): boolean {
  return kind === 'AntivirusBlock';
}

/**
 * SetupWizard — lightweight CLI detection & direct download install.
 *
 * Simplified flow (TK-302 v3):
 *   checking → (CLI found? skip to main) | not_installed
 *   not_installed → user clicks Install → installing (download with progress)
 *   installing → installed → auto-complete
 *   install_failed → retry | skip
 *
 * On Windows, git-bash (PortableGit) is auto-installed as part of the flow.
 * Auth/login is handled separately in Settings (TK-303).
 */
export function SetupWizard() {
  const t = useT();
  const step = useSetupStore((s) => s.step);
  const error = useSetupStore((s) => s.error);
  const cliVersion = useSetupStore((s) => s.cliVersion);
  const setStep = useSetupStore((s) => s.setStep);
  const setError = useSetupStore((s) => s.setError);
  const setCliInfo = useSetupStore((s) => s.setCliInfo);
  const setSetupCompleted = useSettingsStore((s) => s.setSetupCompleted);

  const [downloadPercent, setDownloadPercent] = useState(0);
  const [downloadPhase, setDownloadPhase] = useState<string>('');
  const [deployFailCount, setDeployFailCount] = useState(0);
  const [errorKind, setErrorKind] = useState<string | null>(null);
  const [manualDownloadUrl, setManualDownloadUrl] = useState<string | null>(null);
  const abortRef = useRef(false);

  // NOVA: 自动配置工作目录（Mac: ~/Documents/NOVA, Win: D:\NOVA）
  const autoWorkspace = useCallback(async () => {
    const wd = useSettingsStore.getState().workingDirectory;
    if (wd) return; // 已设置
    try {
      const novaPath = await bridge.getNovaWorkspace();
      useSettingsStore.getState().setWorkingDirectory(novaPath);
      console.log('[NOVA] Auto workspace:', novaPath);
    } catch (e) {
      console.error('[NOVA] Auto workspace failed:', e);
    }
  }, []);

  // NOVA: 安装完成后检查本地模型状态，无 Ollama 则在聊天里引导安装
  const checkOllamaAfterSetup = useCallback(() => {
    const ollamaReady = useProviderStore.getState().ollamaReady;
    const provider = useProviderStore.getState().getActive();
    const hasApiKey = provider?.apiKey && provider.apiKey.trim() !== '';

    // 已有 Ollama 或有 API Key → 不打扰
    if (ollamaReady || hasApiKey) return;

    // 没 Ollama 也没 Key → 打开设置让用户了解状态
    setTimeout(() => {
      useSettingsStore.setState({ settingsOpen: true });
    }, 500);
  }, []);

  // Bug 1: 完成初始化流程（CLI 就绪后）
  const finishSetup = useCallback(() => {
    setSetupCompleted(true);
    autoWorkspace();
    setTimeout(() => checkOllamaAfterSetup(), 500);
  }, [setSetupCompleted, autoWorkspace, checkOllamaAfterSetup]);

  // Bug 1: 自动部署 CLI（融合检测 + 下载）
  const autoDeploy = useCallback(async () => {
    if (abortRef.current) return;
    setStep('installing');
    setError(null);
    setErrorKind(null);
    setDownloadPercent(2);
    setDownloadPhase('');

    // 监听下载进度事件
    const unlistenProgress = await onDownloadProgress((event) => {
      setDownloadPercent(event.percent);
      setDownloadPhase(event.phase);
    });

    try {
      const result: DeployResult = await bridge.autoDeployCli(false);

      if (abortRef.current) return;

      if (result.success) {
        // CLI 已存在或刚下载成功
        setCliInfo(result.version ?? null, result.cliPath ?? null);
        setStep('installed');
        unlistenProgress();
        await bridge.resetDeployFailCount();
        setTimeout(() => finishSetup(), 800);
        return;
      }

      // 失败
      unlistenProgress();
      setError(result.errorMessage ?? '未知错误');
      setErrorKind(result.errorKind);
      setManualDownloadUrl(result.manualDownloadUrl ?? null);

      if (result.suggestManualDownload) {
        setDeployFailCount(3);
      } else {
        const count = await bridge.getDeployFailCount();
        setDeployFailCount(count);
      }

      setStep('install_failed');
    } catch (err) {
      if (abortRef.current) return;
      unlistenProgress();
      const msg = stripAnsi(String(err));
      setError(msg);
      setStep('install_failed');
      try {
        const count = await bridge.getDeployFailCount();
        setDeployFailCount(count);
      } catch { /* ignore */ }
    }
  }, [finishSetup]);

  // Bug 1: Mount 时自动触发部署，不等待用户操作
  useEffect(() => {
    abortRef.current = false;
    autoDeploy();
    return () => { abortRef.current = true; };
  }, []);

  // 重试按钮：强制重新下载
  const handleInstall = useCallback(async () => {
    if (abortRef.current) return;
    setStep('installing');
    setError(null);
    setErrorKind(null);
    setDownloadPercent(2);
    setDownloadPhase('');
    setManualDownloadUrl(null);

    const unlistenProgress = await onDownloadProgress((event) => {
      setDownloadPercent(event.percent);
      setDownloadPhase(event.phase);
    });

    try {
      const result: DeployResult = await bridge.autoDeployCli(true);

      if (abortRef.current) return;

      if (result.success) {
        setCliInfo(result.version ?? null, result.cliPath ?? null);
        setStep('installed');
        unlistenProgress();
        await bridge.resetDeployFailCount();
        setDeployFailCount(0);
        setTimeout(() => finishSetup(), 800);
        return;
      }

      unlistenProgress();
      setError(result.errorMessage ?? '未知错误');
      setErrorKind(result.errorKind);
      setManualDownloadUrl(result.manualDownloadUrl ?? null);
      setDeployFailCount(result.suggestManualDownload ? 3 : deployFailCount + 1);
      setStep('install_failed');
    } catch (err) {
      if (abortRef.current) return;
      unlistenProgress();
      const msg = stripAnsi(String(err));
      setError(msg);
      setStep('install_failed');
    }
  }, [deployFailCount, finishSetup]);

  const handleSkip = useCallback(() => {
    abortRef.current = true;
    setSetupCompleted(true);
  }, []);

  // Phase label for download progress
  const phaseLabel =
    downloadPhase === 'native_version' ? t('setup.nativeVersion')
    : downloadPhase === 'native_manifest' ? t('setup.nativeManifest')
    : downloadPhase === 'native_download' ? t('setup.nativeDownload')
    : downloadPhase === 'native_verify' ? t('setup.nativeVerify')
    : downloadPhase === 'native_install' ? t('setup.nativeInstall')
    : downloadPhase === 'npm_fallback' ? t('setup.npmFallback')
    : downloadPhase === 'node_downloading' ? t('setup.downloadingNode')
    : downloadPhase === 'node_extracting' ? t('setup.extractingNode')
    : downloadPhase === 'node_complete' ? t('setup.preparingEnv')
    : downloadPhase === 'installing' ? t('setup.nativeInstall')
    : downloadPhase === 'git_downloading' ? t('setup.downloadingGit')
    : downloadPhase === 'git_extracting' ? t('setup.extractingGit')
    : downloadPhase === 'git_complete' ? t('setup.preparingEnv')
    : downloadPhase === 'version' ? t('setup.nativeVersion')
    : downloadPhase === 'downloading' ? t('setup.nativeDownload')
    : '';

  return (
    <div className="flex flex-col items-center justify-center h-full text-center">
      <div className="w-full max-w-md">
        {/* Icon — customizable AI avatar */}
        <AiAvatar size="w-20 h-20" rounded="rounded-3xl" className="mb-6 shadow-glow mx-auto" />

        {/* Step: Checking */}
        {step === 'checking' && (
          <div className="space-y-3">
            <div className="flex items-center justify-center gap-2">
              <span className="text-sm font-bold leading-none animate-pulse-soft text-accent">
                /
              </span>
              <span className="text-sm text-text-muted">{t('setup.checking')}</span>
            </div>
          </div>
        )}

        {/* Step: Not Installed */}
        {step === 'not_installed' && (
          <div className="space-y-4">
            <h2 className="text-xl font-semibold text-accent">
              {t('setup.notInstalled')}
            </h2>
            <p className="text-sm text-text-muted leading-relaxed">
              {t('setup.notInstalledDesc')}
            </p>
            <button
              onClick={handleInstall}
              className="px-6 py-3 rounded-xl text-sm font-medium
                bg-accent hover:bg-accent-hover text-text-inverse
                hover:shadow-glow transition-smooth
                flex items-center gap-2 mx-auto cursor-pointer"
            >
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none"
                stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
                <path d="M8 2v9M4.5 7.5L8 11l3.5-3.5M3 14h10" />
              </svg>
              {t('setup.install')}
            </button>

            <button onClick={handleSkip}
              className="text-xs text-text-tertiary hover:text-text-muted
                transition-smooth mt-2 cursor-pointer">
              {t('setup.skip')}
            </button>
          </div>
        )}

        {/* Step: Installing (download progress) — Bug 1: 首次启动自动部署 */}
        {step === 'installing' && (
          <div className="space-y-4">
            <div className="flex items-center justify-center gap-2">
              <span className="text-sm font-bold leading-none animate-pulse-soft text-accent">
                /
              </span>
              <span className="text-sm text-text-muted">
                {downloadPercent <= 5
                  ? t('setup.deploying') // "正在部署 AI 引擎（约 30 秒）"
                  : phaseLabel || t('setup.installing')}
              </span>
            </div>
            {/* Precise progress bar */}
            <div className="h-1.5 rounded-full bg-bg-tertiary overflow-hidden">
              <div
                className="h-full rounded-full bg-accent transition-all duration-300 ease-out"
                style={{ width: `${Math.max(downloadPercent, 2)}%` }}
              />
            </div>
            {downloadPercent > 0 && (
              <span className="text-xs text-text-tertiary">{downloadPercent}%</span>
            )}
          </div>
        )}

        {/* Step: Install Failed — Bug 1: 错误分类 + 手动下载指引 */}
        {step === 'install_failed' && (
          <div className="space-y-4">
            <h2 className="text-xl font-semibold text-red-500">
              {t('setup.installFailed')}
            </h2>

            {/* Bug 1: 具体错误原因 */}
            {errorKind && (
              <p className="text-sm text-text-muted leading-relaxed">
                {isDiskError(errorKind) && t('setup.errorDiskSpace')}
                {isPermissionError(errorKind) && t('setup.errorPermission')}
                {isNetworkError(errorKind) && t('setup.errorNetwork')}
                {isAntivirusError(errorKind) && t('setup.errorAntivirus')}
                {errorKind === 'Unknown' && t('setup.installFailedDesc')}
              </p>
            )}

            {error && (
              <p className="text-xs text-red-400 font-mono bg-red-500/10 p-2 rounded-lg">
                {error}
              </p>
            )}

            {/* 权限错误提示 */}
            {isPermissionError(errorKind) && (
              <p className="text-xs text-amber-500">
                {t('error.permissionHint')}
              </p>
            )}

            {/* 网络错误提示 */}
            {isNetworkError(errorKind) && (
              <p className="text-xs text-amber-500">
                {t('network.firewallHint')}
              </p>
            )}

            {/* 磁盘不足提示 */}
            {isDiskError(errorKind) && (
              <p className="text-xs text-amber-500">
                {t('error.diskSpaceHint')}
              </p>
            )}

            {/* Bug 1: 连续3次失败 → 手动下载指引 */}
            {deployFailCount >= 3 && (
              <div className="bg-amber-500/10 border border-amber-500/30 rounded-xl p-3 space-y-2">
                <p className="text-xs text-amber-500 font-medium">
                  {t('setup.manualDownloadHint')}
                </p>
                {manualDownloadUrl && (
                  <a
                    href={manualDownloadUrl}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-xs text-accent hover:underline inline-block"
                  >
                    {t('setup.manualDownloadLink')}
                  </a>
                )}
              </div>
            )}

            <div className="flex gap-3 justify-center">
              <button onClick={handleInstall}
                className="px-4 py-2 rounded-xl text-sm font-medium
                  bg-accent hover:bg-accent-hover text-text-inverse
                  transition-smooth cursor-pointer">
                {t('setup.retry')}
              </button>
              <button onClick={handleSkip}
                className="px-4 py-2 rounded-xl text-sm font-medium
                  border border-border-subtle text-text-muted
                  hover:bg-bg-tertiary transition-smooth cursor-pointer">
                {t('setup.skip')}
              </button>
            </div>
          </div>
        )}

        {/* Step: Installed (brief confirmation, auto-completes) */}
        {step === 'installed' && (
          <div className="space-y-3 animate-scale-in">
            <div className="flex items-center justify-center gap-2">
              <svg width="20" height="20" viewBox="0 0 20 20" fill="none" className="text-success">
                <circle cx="10" cy="10" r="9" stroke="currentColor" strokeWidth="1.5" />
                <path d="M6 10l3 3 5-6" stroke="currentColor" strokeWidth="1.5"
                  strokeLinecap="round" strokeLinejoin="round" />
              </svg>
              <span className="text-sm text-text-primary font-medium">{t('setup.installed')}</span>
            </div>
            {cliVersion && (
              <p className="text-xs text-text-tertiary">
                {t('setup.version')}: {cliVersion}
              </p>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
