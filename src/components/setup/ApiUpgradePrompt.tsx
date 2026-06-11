interface Props {
  onClose: () => void;
  onUpgrade: () => void;
}

export function ApiUpgradePrompt({ onClose, onUpgrade }: Props) {
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
          升级到 DeepSeek，解锁完整能力
        </h2>

        {/* 本地 vs DeepSeek 对比 */}
        <div style={{ marginBottom: '16px', lineHeight: '1.8' }}>
          <p style={{ color: '#ffd700', fontSize: '12px', margin: '0 0 8px' }}>
            DeepSeek V3 ≈ 如来佛祖级别的代码能力
          </p>
          <p style={{ color: '#5a7288', fontSize: '12px', margin: 0 }}>
            您当前使用的是本地 Ollama 小模型，能基础对话，但
            <span style={{ color: '#00a8e8' }}> 不能读文件、不能改代码、不能用工具</span>。
          </p>
        </div>

        {/* 功能对比 */}
        <div style={{
          background: '#0d1520',
          border: '1px solid #141c28',
          padding: '12px',
          marginBottom: '16px',
          fontSize: '10px'
        }}>
          <p style={{ color: '#3a4a5a', margin: '0 0 8px' }}>
            搭配 DeepSeek API Key 之后：
          </p>
          <p style={{ color: '#5a7288', margin: '0 0 4px' }}>
            ✅ 读文件 &nbsp; ✅ 写代码 &nbsp; ✅ 搜索项目
          </p>
          <p style={{ color: '#5a7288', margin: '0 0 4px' }}>
            ✅ 终端命令 &nbsp; ✅ Git 操作 &nbsp; ✅ 图片理解
          </p>
          <p style={{ color: '#00a8e8', margin: '8px 0 0' }}>
            💰 费用极低：每次对话大约只需几分钱
          </p>
        </div>

        {/* 获取方式 */}
        <div style={{
          background: '#0d1520',
          border: '1px solid #141c28',
          padding: '12px',
          marginBottom: '16px',
          fontSize: '10px'
        }}>
          <p style={{ color: '#3a4a5a', margin: '0 0 4px' }}>
            📝 获取 Key：<span style={{ color: '#5a7288' }}>打开 DeepSeek 官网 → 注册 → API Keys</span>
          </p>
          <p style={{ color: '#3a4a5a', margin: '0 0 4px' }}>
            🔗 地址：<span style={{ color: '#00a8e8' }}>platform.deepseek.com</span>
          </p>
          <p style={{ color: '#3a4a5a', margin: 0 }}>
            ⚡ 新用户注册即送额度，够用很久
          </p>
        </div>

        {/* 退路说明 */}
        <p style={{
          color: '#3a4a5a',
          fontSize: '10px',
          marginBottom: '20px',
          lineHeight: '1.6'
        }}>
          如果暂时不想配，可以继续用本地模型基础对话。
          <br/>随时可以在设置里补填 Key。
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
            继续用本地模型
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
            配置 DeepSeek Key
          </button>
        </div>
      </div>
    </div>
  );
}
