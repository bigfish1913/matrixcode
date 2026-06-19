import React, { useState } from 'react';
import { useConfigStore } from '../stores/configStore';

interface ApproveModeDialogProps {
  onClose: () => void;
}

// Approve modes matching TUI
const APPROVE_MODES = [
  {
    id: 'ask',
    label: 'Ask',
    description: '每次操作都需要确认',
    icon: '❓',
    color: 'text-gray-500',
    details: [
      '工具执行前询问',
      '文件修改前确认',
      '推荐用于敏感操作',
    ],
  },
  {
    id: 'auto',
    label: 'Auto',
    description: '自动执行安全操作',
    icon: '⚡',
    color: 'text-green-600',
    details: [
      '读取操作自动执行',
      '写入操作自动执行',
      '推荐用于日常开发',
    ],
  },
  {
    id: 'strict',
    label: 'Strict',
    description: '严格确认所有操作',
    icon: '🔒',
    color: 'text-red-600',
    details: [
      '所有操作需确认',
      '包括读取文件',
      '推荐用于生产环境',
    ],
  },
];

export function ApproveModeDialog({ onClose }: ApproveModeDialogProps) {
  const [selectedMode, setSelectedMode] = useState<string | null>(null);
  const config = useConfigStore((s) => s.config);
  const updateConfig = useConfigStore((s) => s.updateConfig);

  const currentMode = config?.approve_mode || 'ask';

  // Handle mode selection
  const handleSelectMode = async (modeId: string) => {
    setSelectedMode(modeId);

    // Update config
    try {
      await updateConfig({ approve_mode: modeId });
      console.log(`Approve mode changed to: ${modeId}`);
      onClose();
    } catch (e) {
      console.error('Failed to update approve mode:', e);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
      <div className="bg-card border shadow-lg rounded-lg max-w-md w-full overflow-hidden">
        {/* Header */}
        <div className="p-4 border-b bg-muted/30">
          <div className="flex items-center justify-between">
            <h3 className="text-lg font-semibold flex items-center gap-2">
              <span>⚡</span>
              <span>Approve Mode</span>
            </h3>
            <button
              onClick={onClose}
              className="text-muted-foreground hover:text-foreground p-1 rounded hover:bg-accent transition-colors"
            >
              ✕
            </button>
          </div>
          <p className="text-sm text-muted-foreground mt-1">
            选择 Agent 执行操作的批准模式
          </p>
        </div>

        {/* Current mode indicator */}
        <div className="px-4 py-2 bg-muted/20 border-b">
          <div className="flex items-center gap-2 text-xs">
            <span className="text-muted-foreground">当前模式:</span>
            <span className={`px-2 py-0.5 rounded font-mono ${APPROVE_MODES.find(m => m.id === currentMode)?.color}`}>
              {APPROVE_MODES.find(m => m.id === currentMode)?.label}
            </span>
          </div>
        </div>

        {/* Mode options */}
        <div className="p-4 space-y-3">
          {APPROVE_MODES.map((mode) => (
            <button
              key={mode.id}
              onClick={() => handleSelectMode(mode.id)}
              disabled={selectedMode !== null}
              className={`w-full p-4 rounded-lg border transition-all ${
                currentMode === mode.id
                  ? 'border-primary bg-primary/10'
                  : 'border-border hover:border-primary/50 hover:bg-accent/30'
              } ${selectedMode === mode.id ? 'animate-pulse' : ''}`}
            >
              <div className="flex items-start gap-3">
                <span className={`text-2xl ${mode.color}`}>{mode.icon}</span>
                <div className="flex-1">
                  <div className="flex items-center gap-2">
                    <span className="font-semibold">{mode.label}</span>
                    {currentMode === mode.id && (
                      <span className="px-1.5 py-0.5 bg-primary text-primary-foreground text-xs rounded">
                        当前
                      </span>
                    )}
                  </div>
                  <p className="text-sm text-muted-foreground mt-1">
                    {mode.description}
                  </p>
                  <ul className="mt-2 space-y-1">
                    {mode.details.map((detail, idx) => (
                      <li key={idx} className="text-xs text-muted-foreground flex items-center gap-1">
                        <span className="text-muted-foreground/50">•</span>
                        {detail}
                      </li>
                    ))}
                  </ul>
                </div>
              </div>
            </button>
          ))}
        </div>

        {/* Footer */}
        <div className="p-4 border-t bg-muted/30">
          <div className="flex gap-2">
            <button
              onClick={onClose}
              className="flex-1 px-4 py-2 bg-muted text-muted-foreground rounded-lg text-sm hover:bg-accent transition-colors"
            >
              取消
            </button>
            <div className="flex-1 flex items-center justify-center text-xs text-muted-foreground">
              <kbd className="px-1.5 py-0.5 bg-muted rounded">Alt+M</kbd>
              <span className="ml-1">快捷切换</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}