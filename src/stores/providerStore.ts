import { create } from 'zustand';
import { bridge, type ProvidersFile } from '../lib/tauri-bridge';

export interface ModelMapping {
  /** Standard tier ('opus'|'sonnet'|'haiku') or a specific model ID for direct mapping */
  tier: string;
  providerModel: string;
}

export interface ApiProvider {
  id: string;
  name: string;
  baseUrl: string;
  apiFormat: 'anthropic' | 'openai';
  apiKey?: string;
  modelMappings: ModelMapping[];
  extra_env?: Record<string, string>;
  proxyUrl?: string;
  preset?: string;
  createdAt: number;
  updatedAt: number;
}

interface ProviderState {
  providers: ApiProvider[];
  activeProviderId: string | null;
  loaded: boolean;
  /** Ollama 是否可用 */
  ollamaReady: boolean;
  /** Ollama 可用模型列表 */
  ollamaModels: Array<{ name: string; size: number }>;
  /** 是否已经弹出过首次任务结束后的 DeepSeek 引导 */
  deepseekPromptShown: boolean;

  load: () => Promise<void>;
  save: () => Promise<void>;
  addProvider: (p: Omit<ApiProvider, 'id' | 'createdAt' | 'updatedAt'>) => void;
  updateProvider: (id: string, patch: Partial<ApiProvider>) => void;
  deleteProvider: (id: string) => void;
  setActive: (id: string | null) => void;
  getActive: () => ApiProvider | null;
  /** 检测 Ollama 并自动创建本地 provider */
  detectOllama: () => Promise<void>;
  /** 标记 DeepSeek 引导已显示 */
  markDeepseekPromptShown: () => void;
}

function generateId(): string {
  return Math.random().toString(36).slice(2, 10) + Date.now().toString(36);
}

let _saveTimer: ReturnType<typeof setTimeout> | undefined;

function debouncedSave(state: ProviderState) {
  clearTimeout(_saveTimer);
  _saveTimer = setTimeout(() => {
    state.save().catch((e) => console.error('[providerStore] save failed:', e));
  }, 500);
}

export const useProviderStore = create<ProviderState>()((set, get) => ({
  providers: [],
  activeProviderId: null,
  loaded: false,
  ollamaReady: false,
  ollamaModels: [],
  deepseekPromptShown: localStorage.getItem('nova-deepseek-prompt-shown') === '1',

  load: async () => {
    try {
      const data = await bridge.loadProviders();

      // If providers.json is empty, try migrating from old settingsStore data
      if (data.providers.length === 0) {
        const migrated = migrateFromSettingsStore();
        if (migrated) {
          data.providers = [migrated];
          data.activeProviderId = migrated.id;
          await bridge.saveProviders(data);
          console.log('[providerStore] Migrated old API settings to provider:', migrated.name);
        } else {
          // NOVA: 全新安装 — 自动创建 Agnes AI 免费 provider
          const agnesProvider: ApiProvider = {
            id: generateId(),
            name: "Agnes AI（免费无限）",
            baseUrl: "https://apihub.agnes-ai.com/v1",
            apiFormat: "openai",
            apiKey: "sk-o5BUpyY2j6D57I2F1XqSmOt1Qnr1ffiz4Y1TKpX4MKBmJNbt",
            modelMappings: [
              { tier: "opus", providerModel: "agnes-2.0-flash" },
              { tier: "sonnet", providerModel: "agnes-2.0-flash" },
              { tier: "haiku", providerModel: "agnes-2.0-flash" },
            ],
            preset: "agnes",
            createdAt: Date.now(),
            updatedAt: Date.now(),
          };
          data.providers = [agnesProvider];
          data.activeProviderId = agnesProvider.id;
          await bridge.saveProviders(data);
          console.log("[providerStore] Fresh install -- auto-created Agnes AI provider");
        }
      }

      // NOVA: 即使有旧 providers，检查是否有可用的（有 API key 或 Ollama）
      // 如果没有，自动插入 Agnes AI 作为推荐默认
      const hasUsableProvider = data.providers.some(
        (p: any) => p.apiKey || p.preset === 'ollama'
      );

      if (!hasUsableProvider) {
        const existingAgnes = data.providers.find((p: any) => p.preset === 'agnes');
        if (!existingAgnes) {
          const agnesProvider: ApiProvider = {
            id: generateId(),
            name: "Agnes AI（免费无限）",
            baseUrl: "https://apihub.agnes-ai.com/v1",
            apiFormat: "openai",
            apiKey: "sk-o5BUpyY2j6D57I2F1XqSmOt1Qnr1ffiz4Y1TKpX4MKBmJNbt",
            modelMappings: [
              { tier: "opus", providerModel: "agnes-2.0-flash" },
              { tier: "sonnet", providerModel: "agnes-2.0-flash" },
              { tier: "haiku", providerModel: "agnes-2.0-flash" },
            ],
            preset: "agnes",
            createdAt: Date.now(),
            updatedAt: Date.now(),
          };
          data.providers.unshift(agnesProvider);
          data.activeProviderId = agnesProvider.id;
          await bridge.saveProviders(data);
          console.log("[providerStore] Added Agnes AI as default (no usable provider found)");
        } else if (data.activeProviderId !== existingAgnes.id) {
          // Agnes 已存在但未激活，切换过去
          data.activeProviderId = existingAgnes.id;
          await bridge.saveProviders(data);
          console.log("[providerStore] Switched to existing Agnes AI provider");
        }
      }

      set({
        providers: data.providers as ApiProvider[],
        activeProviderId: data.activeProviderId,
        loaded: true,
      });
    } catch (e) {
      console.error('[providerStore] load failed:', e);
      set({ loaded: true });
    }
  },

  save: async () => {
    const { providers, activeProviderId } = get();
    const data: ProvidersFile = {
      version: 1,
      activeProviderId,
      providers,
    };
    await bridge.saveProviders(data);
  },

  addProvider: (p) => {
    const now = Date.now();
    const newProvider: ApiProvider = {
      ...p,
      id: generateId(),
      createdAt: now,
      updatedAt: now,
    };
    set((s) => ({ providers: [...s.providers, newProvider] }));
    debouncedSave(get());
  },

  updateProvider: (id, patch) => {
    set((s) => ({
      providers: s.providers.map((p) =>
        p.id === id ? { ...p, ...patch, updatedAt: Date.now() } : p,
      ),
    }));
    debouncedSave(get());
  },

  deleteProvider: (id) => {
    set((s) => ({
      providers: s.providers.filter((p) => p.id !== id),
      activeProviderId: s.activeProviderId === id ? null : s.activeProviderId,
    }));
    debouncedSave(get());
  },

  setActive: (id) => {
    set({ activeProviderId: id });
    debouncedSave(get());
  },

  getActive: () => {
    const { providers, activeProviderId } = get();
    if (!activeProviderId) return null;
    return providers.find((p) => p.id === activeProviderId) ?? null;
  },

  /** 检测本地 Ollama 是否可用，创建本地 provider */
  detectOllama: async () => {
    try {
      const status = await bridge.checkOllama();
      set({
        ollamaReady: status.running,
        ollamaModels: status.models,
      });
      if (status.running) {
        // 自动创建或更新本地 Ollama provider
        const { providers } = get();
        const existingLocal = providers.find((p) => p.preset === 'ollama');
        if (!existingLocal) {
          const localProvider: ApiProvider = {
            id: generateId(),
            name: '本地模型 (Ollama)',
            baseUrl: 'http://localhost:11434/v1',
            apiFormat: 'openai',
            apiKey: 'ollama',
            modelMappings: status.models.slice(0, 5).map((m: { name: string; size: number }) => ({
              tier: 'opus',
              providerModel: m.name,
            })),
            preset: 'ollama',
            createdAt: Date.now(),
            updatedAt: Date.now(),
          };
          set((s) => ({
            providers: [...s.providers, localProvider],
            // 没有其他 provider 时设为默认
            activeProviderId: s.activeProviderId ?? localProvider.id,
          }));
          debouncedSave(get());
          console.log('[providerStore] Created local Ollama provider');
        } else {
          // 更新已有本地 provider 的模型列表
          const updatedModels = status.models.slice(0, 5).map((m: { name: string; size: number }) => ({
            tier: 'opus' as const,
            providerModel: m.name,
          }));
          get().updateProvider(existingLocal.id, { modelMappings: updatedModels });
        }
      }
    } catch (e) {
      console.log('[providerStore] Ollama detection skipped:', e);
      set({ ollamaReady: false, ollamaModels: [] });
    }
  },

  markDeepseekPromptShown: () => {
    localStorage.setItem('nova-deepseek-prompt-shown', '1');
    set({ deepseekPromptShown: true });
  },
}));

/**
 * Migrate from old settingsStore API fields to a new ApiProvider.
 * Returns null if no old config exists or mode is 'inherit'.
 */
function migrateFromSettingsStore(): ApiProvider | null {
  try {
    // Read old settings from localStorage (settingsStore persists there)
    const raw = localStorage.getItem('tokenicode-settings');
    if (!raw) return null;
    const parsed = JSON.parse(raw);
    const state = parsed?.state;
    if (!state) return null;

    const mode = state.apiProviderMode;
    if (!mode || mode === 'inherit') return null;

    const now = Date.now();
    const provider: ApiProvider = {
      id: generateId(),
      name: state.customProviderName || (mode === 'official' ? 'Anthropic (官方)' : 'Custom'),
      baseUrl: mode === 'official' ? 'https://api.anthropic.com' : (state.customProviderBaseUrl || ''),
      apiFormat: (state.customProviderApiFormat || 'anthropic') as 'anthropic' | 'openai',
      modelMappings: Array.isArray(state.customProviderModelMappings)
        ? state.customProviderModelMappings.map((m: { tier: string; providerModel: string }) => ({
            tier: m.tier as 'opus' | 'sonnet' | 'haiku',
            providerModel: m.providerModel,
          }))
        : [],
      createdAt: now,
      updatedAt: now,
    };

    return provider;
  } catch {
    return null;
  }
}
