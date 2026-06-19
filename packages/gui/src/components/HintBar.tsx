import React from 'react';
import { useChatStore } from '../stores/chatStore';

interface HintBarProps {
  message?: string;
}

// Activity-specific hints matching TUI
const ACTIVITY_HINTS: Record<string, string[]> = {
  thinking: ['Esc 取消', '思考中...'],
  reading: ['Esc 取消', '📖 读取文件'],
  writing: ['Esc 取消', '📝 写入文件'],
  editing: ['Esc 取消', '✏️ 编辑文件'],
  searching: ['Esc 取消', '🔍 搜索中'],
  tool: ['Esc 取消', '🔧 执行工具'],
  websearch: ['Esc 取消', '🌐 Web 搜索'],
  webfetch: ['Esc 取消', '🔗 获取网页'],
};

// Default hints cycling
const DEFAULT_HINTS_CYCLE = [
  '⌨️ / 命令栏',
  'Ctrl+D 调试面板',
  'Alt+W 工作流',
  '↑↓ 输入历史',
];

export function HintBar({ message }: HintBarProps) {
  const activity = useChatStore((s) => s.activity);
  const status = useChatStore((s) => s.status);
  const pendingMessages = useChatStore((s) => s.pendingMessages);
  const askQuestion = useChatStore((s) => s.askQuestion);

  // Determine what hint to show
  let displayHint = '';

  // 1. Explicit message (highest priority)
  if (message) {
    displayHint = message;
  }
  // 2. Ask question waiting
  else if (askQuestion?.isVisible) {
    displayHint = '❓ 选择选项后按 Enter 提交 | Esc 取消';
  }
  // 3. Pending messages in queue
  else if (pendingMessages.length > 0) {
    displayHint = `📥 ${pendingMessages.length} 条消息排队中 | Shift+Esc 清除队列`;
  }
  // 4. Activity-specific hint
  else if (activity.type !== 'idle' && ACTIVITY_HINTS[activity.type]) {
    const hints = ACTIVITY_HINTS[activity.type];
    displayHint = hints.join(' | ');
    if (activity.detail) {
      displayHint += ` (${activity.detail})`;
    }
  }
  // 5. Running status
  else if (status === 'running') {
    displayHint = '⚡ Agent 执行中... | Esc 中断';
  }
  // 6. Cycle through default hints (based on time)
  else {
    const idx = Math.floor(Date.now() / 5000) % DEFAULT_HINTS_CYCLE.length;
    displayHint = DEFAULT_HINTS_CYCLE[idx];
  }

  return (
    <div className="px-4 py-1 bg-muted/30 border-b text-xs text-muted-foreground overflow-hidden">
      <div className="animate-pulse-subtle">
        {displayHint}
      </div>
    </div>
  );
}