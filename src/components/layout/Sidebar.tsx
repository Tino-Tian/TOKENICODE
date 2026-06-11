import { useSettingsStore, MODEL_OPTIONS } from '../../stores/settingsStore';
import { useChatStore, useActiveTab } from '../../stores/chatStore';
import { useSessionStore } from '../../stores/sessionStore';
import { ConversationList } from '../conversations/ConversationList';
import { useT } from '../../lib/i18n';
import { useAgentStore } from '../../stores/agentStore';
import { IS_ALPHA } from '../../lib/edition';

/** Map raw model ID to friendly display name */
function getModelDisplayName(modelId: string): string {
  const option = MODEL_OPTIONS.find((m) => modelId.includes(m.id));
  return option?.short || modelId;
}

/** Format token count: 1234 → "1.2k", 123456 → "123k", 1234567 → "1.2M" */
function formatTokenCount(n: number): string {
  if (n < 1000) return String(n);
  if (n < 100_000) return (n / 1000).toFixed(1) + 'k';
  if (n < 1_000_000) return Math.round(n / 1000) + 'k';
  return (n / 1_000_000).toFixed(1) + 'M';
}

export function Sidebar() {
  const toggleSidebar = useSettingsStore((s) => s.toggleSidebar);
  const toggleSettings = useSettingsStore((s) => s.toggleSettings);
  const updateAvailable = useSettingsStore((s) => s.updateAvailable);
  const cliUpdateAvailable = useSettingsStore((s) => s.cliUpdateAvailable);
  const sessionMeta = useActiveTab((t) => t.sessionMeta);
  const sessionStatus = useActiveTab((t) => t.sessionStatus);
  const t = useT();

  // Window dragging handled via CSS -webkit-app-region: drag on the top strip

  return (
    <div className="flex flex-col h-full pt-8 pb-4">
      {/* Logo area */}
      <div
        className="flex items-center justify-between mb-6 px-5 cursor-default">
        <div className="flex items-center pointer-events-none">
          {IS_ALPHA ? (
            <>
              <span className="text-[14px] font-bold tracking-tight text-text-primary">
                TC<span style={{color: 'var(--color-accent)'}}>/</span>Alpha
              </span>
              <span className="ml-1.5 px-1.5 py-0.5 rounded text-[9px] font-semibold uppercase
                bg-accent/15 text-accent leading-none">
                alpha
              </span>
            </>
          ) : (
            /* NOVA: Claude Code + 克劳德 */
            <div className="flex flex-col pointer-events-none select-none">
              <span className="text-[15px] font-bold tracking-tight text-text-primary leading-tight">
                Claude<span style={{color: 'var(--color-accent)'}}> Code</span>
              </span>
              <span className="text-[10px] text-text-tertiary leading-tight mt-0.5">
                克劳德
              </span>
            </div>
          )}
        </div>
        <button onClick={toggleSidebar}
          className="p-1.5 rounded-lg hover:bg-bg-tertiary text-text-tertiary
            transition-smooth" title={t('sidebar.hide')}>
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none"
            stroke="currentColor" strokeWidth="1.5">
            <path d="M10 4L6 8L10 12" />
          </svg>
        </button>
      </div>

      {/* New Chat — navigate to WelcomeScreen where user picks a folder */}
      <div className="px-3">
      <button onClick={() => {
        // Save current session to cache before switching
        const currentTabId = useSessionStore.getState().selectedSessionId;
        if (currentTabId) {
          useChatStore.getState().saveToCache(currentTabId);
          useAgentStore.getState().saveToCache(currentTabId);
        }

        // Deselect current session FIRST so background stream routing works
        useSessionStore.getState().setSelectedSession(null);

        // Clear working directory so ChatPanel shows WelcomeScreen
        useSettingsStore.getState().setWorkingDirectory('');
      }}
        {...(import.meta.env.DEV && { 'data-testid': 'new-session-button' })}
        className="w-full py-2.5 px-4 rounded-[20px] text-sm font-medium
          bg-accent hover:bg-accent-hover text-text-inverse
          hover:shadow-glow transition-smooth mb-4
          flex items-center justify-center gap-2">
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none"
          stroke="currentColor" strokeWidth="2" strokeLinecap="round">
          <path d="M8 3v10M3 8h10" />
        </svg>
        {t('sidebar.newChat')}
      </button>

      {/* Current Session — compressed single-line card */}
      {(sessionMeta.stdinId || sessionMeta.sessionId) && (
        <div className="px-3 py-2 rounded-xl bg-bg-secondary border border-border-subtle mb-3
          flex items-center gap-2"
          {...(import.meta.env.DEV && { 'data-testid': 'current-session-card' })}>
          <span className={`w-2 h-2 rounded-full flex-shrink-0 transition-smooth
            ${sessionStatus === 'running'
              ? 'bg-success shadow-[0_0_8px_var(--color-accent-glow)] animate-pulse-soft'
              : sessionStatus === 'completed' ? 'bg-success'
              : sessionStatus === 'error' ? 'bg-error'
              : 'bg-text-tertiary'}`} />
          <span className="text-xs font-medium text-text-primary truncate">
            {sessionMeta.model ? getModelDisplayName(sessionMeta.model) : 'Claude'}
          </span>
          {(sessionMeta.totalInputTokens || sessionMeta.totalOutputTokens
            || sessionMeta.inputTokens || sessionMeta.outputTokens) ? (
            <span className="text-[10px] text-text-tertiary font-mono flex items-center gap-1 ml-auto flex-shrink-0">
              <span>↑{formatTokenCount(sessionMeta.totalInputTokens || sessionMeta.inputTokens || 0)}</span>
              <span>↓{formatTokenCount(sessionMeta.totalOutputTokens || sessionMeta.outputTokens || 0)}</span>
            </span>
          ) : (
            <span className="text-[10px] text-text-tertiary capitalize ml-auto flex-shrink-0">{sessionStatus}</span>
          )}
        </div>
      )}
      </div>

      {/* Conversation History */}
      <div className="flex-1 overflow-y-auto overflow-x-hidden min-h-0 -mr-1.5 pr-1.5">
        <ConversationList />
      </div>

      {/* Footer */}
      <div className="pt-3 mt-3 border-t border-border-subtle px-3">
        <button onClick={toggleSettings}
          {...(import.meta.env.DEV && { 'data-testid': 'settings-button' })}
          className="w-full flex items-center gap-2.5 px-3 py-2 rounded-xl
            text-sm text-text-muted hover:bg-bg-secondary hover:text-text-primary
            transition-smooth">
          <div className="relative">
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none"
              stroke="currentColor" strokeWidth="1.5">
              <circle cx="8" cy="8" r="2" />
              <path d="M8 1v2M8 13v2M1 8h2M13 8h2M3.05 3.05l1.41 1.41M11.54 11.54l1.41 1.41M3.05 12.95l1.41-1.41M11.54 4.46l1.41-1.41" />
            </svg>
            {(updateAvailable || cliUpdateAvailable) && (
              <span className={`absolute -top-1 -right-1.5 w-2 h-2 rounded-full
                border-[1.5px] border-bg-sidebar ${cliUpdateAvailable ? 'bg-red-500' : 'bg-green-500'}`} />
            )}
          </div>
          {t('settings.title')}
        </button>
      </div>
    </div>
  );
}
