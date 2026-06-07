import { useState, useEffect } from 'react';
import { useSettingsStore } from '../../stores/settingsStore';

interface Props {
  onClose: () => void;
  onUpgrade: () => void;
}

/** Token usage from the setup task (estimated, updated at runtime) */
function estimateTokens(): { zhipuTokens: number; deepseekTokens: number; deepseekCost: string } {
  // These are placeholder values — in production, read from actual chat session
  return {
    zhipuTokens: 1500,
    deepseekTokens: 1200,
    deepseekCost: '0.002',
  };
}

export function ApiUpgradePrompt({ onClose, onUpgrade }: Props) {
  const locale = useSettingsStore((s) => s.locale);
  const isZh = locale === 'zh';
  const { zhipuTokens, deepseekTokens, deepseekCost } = estimateTokens();

  return (
    <div style={{
      position: 'fixed',
      inset: 0,
      zIndex: 10000,
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      background: 'rgba(0,0,0,0.7)',
      fontFamily: 'monospace'
    }}>
      <div style={{
        background: '#080c14',
        border: '1px solid #141c28',
        padding: '28px 32px',
        maxWidth: '440px',
        width: '100%',
        margin: '0 16px'
      }}>
        {/* 标题 */}
        <h2 style={{
          color: '#00a8e8',
          fontSize: '14px',
          margin: '0 0 16px 0',
          letterSpacing: '2px',
          fontWeight: 700
        }}>
          是否更换 DeepSeek API Key？
        </h2>

        {/* 煤灰 vs 如来 */}
        <div style={{ marginBottom: '16px', lineHeight: '1.8' }}>
          <p style={{ color: '#5a7288', fontSize: '12px', margin: '0 0 8px' }}>
            DeepSeek V3 约等于 <span style={{ color: '#ffd700' }}>如来佛祖</span>。
          </p>
          <p style={{ color: '#3a4a5a', fontSize: '12px', margin: '0 0 8px' }}>
            您现在用的智谱 GLM 免费版约等于
            <span style={{ color: '#4a4a4a' }}> 太上老君炼丹炉里的煤灰</span>。
          </p>
        </div>

        {/* 量化对比 */}
        <div style={{
          background: '#0d1520',
          border: '1px solid #141c28',
          padding: '12px',
          marginBottom: '16px',
          fontSize: '10px'
        }}>
          <p style={{ color: '#3a4a5a', margin: '0 0 6px' }}>
            以本次引导任务为例：
          </p>
          <p style={{ color: '#5a7288', margin: '0 0 4px' }}>
            智谱 GLM：约 {zhipuTokens.toLocaleString()} Tokens（免费，不花钱）
          </p>
          <p style={{ color: '#00a8e8', margin: 0 }}>
            换成 DeepSeek：约 {deepseekTokens.toLocaleString()} Tokens（约 ¥{deepseekCost}）
          </p>
        </div>

        {/* 退路 */}
        <p style={{
          color: '#3a4a5a',
          fontSize: '10px',
          marginBottom: '20px',
          lineHeight: '1.6'
        }}>
          ——当然，您也可以选择不换。
        </p>

        {/* 按钮 */}
        <div style={{
          display: 'flex',
          gap: '8px',
          justifyContent: 'flex-end'
        }}>
          <button
            onClick={onClose}
            style={{
              border: '1px solid #1a2030',
              padding: '8px 18px',
              color: '#3a4a5a',
              fontFamily: 'monospace',
              fontSize: '9px',
              background: 'transparent',
              cursor: 'pointer'
            }}
          >
            不换，继续用免费版
          </button>
          <button
            onClick={onUpgrade}
            style={{
              background: '#00a8e8',
              padding: '8px 18px',
              color: '#080c14',
              fontFamily: 'monospace',
              fontSize: '9px',
              fontWeight: 700,
              border: 'none',
              letterSpacing: '1px',
              cursor: 'pointer'
            }}
          >
            更换 DeepSeek Key
          </button>
        </div>
      </div>
    </div>
  );
}
