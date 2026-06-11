import { useState, useCallback } from 'react';
import { bridge } from '../../lib/tauri-bridge';
import { useProviderStore } from '../../stores/providerStore';

/** NOVA: 一键安装 Ollama 按钮 — 调用内嵌的 Ollama 二进制安装到系统 */
export function OllamaInstallButton() {
  const [state, setState] = useState<'idle' | 'installing' | 'done' | 'error' | 'cancelled'>('idle');
  const [errMsg, setErrMsg] = useState('');

  const handleInstall = useCallback(async () => {
    setState('installing');
    setErrMsg('');
    try {
      const result = await bridge.installBundledOllama();
      if (result === 'already_installed' || result === 'installed') {
        setState('done');
        // 等服务启动 — ollama serve 需要几秒初始化，轮询检测
        for (let i = 0; i < 6; i++) {
          await new Promise(r => setTimeout(r, 1500));
          useProviderStore.getState().detectOllama();
          const { ollamaReady } = useProviderStore.getState();
          if (ollamaReady) break;
        }
      } else {
        setState('error');
        setErrMsg(`未知结果: ${result}`);
      }
    } catch (e: any) {
      const errStr = String(e);
      if (errStr.includes('cancelled') || errStr.includes('(-128)')) {
        setState('cancelled');
      } else {
        setState('error');
        setErrMsg(errStr.slice(0, 200));
      }
    }
  }, []);

  if (state === 'done') {
    return (
      <span className="px-3 py-1.5 rounded-lg text-[10px] font-medium
        bg-green-500/10 border border-green-500/20 text-green-400">
        ✅ Ollama 已安装
      </span>
    );
  }

  if (state === 'cancelled') {
    return (
      <button
        onClick={handleInstall}
        className="px-3 py-1.5 rounded-lg text-[10px] font-medium
          bg-accent/10 border border-accent/20 text-accent
          hover:bg-accent/20 transition-smooth cursor-pointer"
      >
        重新安装 Ollama
      </button>
    );
  }

  if (state === 'error') {
    return (
      <div className="flex flex-col gap-1">
        <button
          onClick={handleInstall}
          className="px-3 py-1.5 rounded-lg text-[10px] font-medium
            bg-red-500/10 border border-red-500/20 text-red-400
            hover:bg-red-500/20 transition-smooth cursor-pointer"
        >
          重试安装
        </button>
        {errMsg && (
          <span className="text-[9px] text-red-400/60">{errMsg}</span>
        )}
      </div>
    );
  }

  return (
    <button
      onClick={handleInstall}
      disabled={state === 'installing'}
      className="px-3 py-1.5 rounded-lg text-[10px] font-medium
        bg-accent text-text-inverse
        hover:bg-accent-hover transition-smooth cursor-pointer
        disabled:opacity-50 disabled:cursor-not-allowed
        flex items-center gap-1.5"
    >
      {state === 'installing' && (
        <span className="w-3 h-3 border-2 border-white/30 border-t-white
          rounded-full animate-spin" />
      )}
      {state === 'installing' ? '正在安装...' : '一键安装 Ollama'}
    </button>
  );
}
