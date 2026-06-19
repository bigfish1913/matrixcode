import React, { useState, useEffect, useRef } from 'react';
import { useChatStore } from '../stores/chatStore';
import { useConfigStore } from '../stores/configStore';
import { useSessionStore } from '../stores/sessionStore';
import { useToastContext } from '../contexts/ToastContext';

// Command definition matching TUI CommandRegistry
interface Command {
  name: string;
  aliases?: string[];
  description: string;
  category: 'general' | 'session' | 'config' | 'tools' | 'debug';
  action?: () => void | Promise<void>;
  requiresArg?: boolean;
}

const COMMANDS: Command[] = [
  // General commands
  { name: '/help', aliases: ['/?'], description: '显示帮助信息', category: 'general' },
  { name: '/shortcuts', aliases: ['/keys'], description: '显示完整快捷键列表', category: 'general' },
  { name: '/clear', description: '清空消息', category: 'general' },
  { name: '/exit', aliases: ['/quit'], description: '退出程序', category: 'general' },
  { name: '/clipboard', aliases: ['/clip'], description: '剪贴板历史', category: 'general' },
  { name: '/search', aliases: ['/find'], description: '搜索消息', category: 'general' },
  { name: '/theme', description: '切换主题', category: 'general' },
  { name: '/batch', description: '批量操作', category: 'general' },

  // Session commands
  { name: '/new', description: '新建会话', category: 'session' },
  { name: '/save', description: '保存会话', category: 'session' },
  { name: '/sessions', aliases: ['/history'], description: '显示会话历史', category: 'session' },
  { name: '/retry', description: '重试最后消息', category: 'session' },

  // Config commands
  { name: '/mode', description: '切换批准模式 (ask/auto/strict)', category: 'config', requiresArg: false },
  { name: '/model', description: '显示/切换模型', category: 'config' },
  { name: '/config', description: '显示配置信息', category: 'config' },

  // Tools commands
  { name: '/tools', description: '列出可用工具', category: 'tools' },
  { name: '/skills', description: '列出已加载技能', category: 'tools' },
  { name: '/compact', description: '压缩上下文', category: 'tools' },
  { name: '/memory', description: '查看/管理记忆', category: 'tools' },

  // Debug commands
  { name: '/debug', description: '切换调试模式', category: 'debug' },
  { name: '/workflow', aliases: ['/wf'], description: '工作流管理', category: 'debug' },
  { name: '/mcp', description: 'MCP服务器状态', category: 'debug' },
  { name: '/stats', aliases: ['/token'], description: 'Token统计', category: 'debug' },
];

interface CommandBarProps {
  onSubmitCommand: (command: string) => void;
  onClose: () => void;
}

export function CommandBar({ onSubmitCommand, onClose }: CommandBarProps) {
  const [input, setInput] = useState('/');
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [filteredCommands, setFilteredCommands] = useState<Command[]>(COMMANDS);
  const inputRef = useRef<HTMLInputElement>(null);

  const clearMessages = useChatStore((s) => s.clearMessages);
  const toggleDebugPanel = useChatStore((s) => s.toggleDebugPanel);
  const toggleWorkflowPanel = useChatStore((s) => s.toggleWorkflowPanel);
  const retryLastMessage = useChatStore((s) => s.retryLastMessage);
  const config = useConfigStore((s) => s.config);
  const toast = useToastContext();

  // Focus input on mount
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // Filter commands based on input
  useEffect(() => {
    const query = input.toLowerCase().slice(1); // Remove leading '/'
    if (!query) {
      setFilteredCommands(COMMANDS);
    } else {
      const filtered = COMMANDS.filter(cmd =>
        cmd.name.toLowerCase().includes(query) ||
        cmd.description.toLowerCase().includes(query) ||
        cmd.aliases?.some(a => a.toLowerCase().includes(query))
      );
      setFilteredCommands(filtered);
    }
    setSelectedIndex(0);
  }, [input]);

  // Handle keyboard navigation
  const handleKeyDown = (e: React.KeyboardEvent) => {
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        setSelectedIndex(i => Math.min(i + 1, filteredCommands.length - 1));
        break;
      case 'ArrowUp':
        e.preventDefault();
        setSelectedIndex(i => Math.max(i - 1, 0));
        break;
      case 'Enter':
        e.preventDefault();
        if (filteredCommands[selectedIndex]) {
          executeCommand(filteredCommands[selectedIndex]);
        }
        break;
      case 'Escape':
        e.preventDefault();
        onClose();
        break;
      case 'Tab':
        e.preventDefault();
        if (filteredCommands[selectedIndex]) {
          setInput(filteredCommands[selectedIndex].name);
        }
        break;
    }
  };

  // Execute command
  const executeCommand = async (cmd: Command) => {
    onSubmitCommand(cmd.name);

    // Get store methods
    const createSession = useSessionStore.getState().createSession;
    const clearMessages = useChatStore.getState().clearMessages;

    // Execute command action
    switch (cmd.name) {
      case '/clear':
        clearMessages();
        toast.addToast({ type: 'success', message: '已清空消息' });
        break;
      case '/debug':
        toggleDebugPanel();
        break;
      case '/workflow':
        toggleWorkflowPanel();
        break;
      case '/retry':
        await retryLastMessage();
        toast.addToast({ type: 'info', message: '正在重试最后消息' });
        break;
      case '/new':
        createSession();
        clearMessages();
        toast.addToast({ type: 'success', message: '已创建新会话' });
        break;
      case '/mode':
        // Cycle through modes: auto -> ask -> strict -> auto
        const modes = ['auto', 'ask', 'strict'];
        const currentMode = config?.approve_mode || 'auto';
        const currentIdx = modes.indexOf(currentMode);
        const nextMode = modes[(currentIdx + 1) % modes.length];
        toast.addToast({ type: 'info', message: `切换模式: ${currentMode} → ${nextMode} (TODO)` });
        console.log(`Switching mode from ${currentMode} to ${nextMode}`);
        // TODO: Call update_config to change mode
        break;
      case '/model':
        // Show model info
        toast.addToast({ type: 'info', message: `当前模型: ${config?.model || 'claude'}` });
        console.log('Model info - current:', config?.model);
        break;
      case '/help':
      case '/shortcuts':
        // Show help dialog (already handled by parent)
        break;
      case '/sessions':
      case '/history':
        // Show session switcher dialog
        toast.addToast({ type: 'info', message: '会话历史 (TODO)' });
        console.log('Session history');
        break;
      case '/save':
        // Save current session
        toast.addToast({ type: 'info', message: '保存会话 (TODO)' });
        console.log('Save session');
        break;
      case '/exit':
      case '/quit':
        toast.addToast({ type: 'warning', message: '退出程序 (TODO)' });
        console.log('Exit program');
        break;
      default:
        // Commands that need backend support
        toast.addToast({ type: 'info', message: `命令 ${cmd.name} 需要后端支持` });
        console.log(`Command ${cmd.name} requires backend support`);
        break;
    }

    onClose();
  };

  // Category colors
  const categoryColors: Record<string, string> = {
    general: 'text-blue-500',
    session: 'text-green-500',
    config: 'text-yellow-500',
    tools: 'text-purple-500',
    debug: 'text-red-500',
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-start justify-center z-50 p-4 pt-[20%]" onClick={(e) => {
      // Close on background click
      if (e.target === e.currentTarget) {
        onClose();
      }
    }}>
      <div className="bg-card border shadow-lg rounded-lg max-w-lg w-full overflow-hidden">
        {/* Input */}
        <div className="p-3 border-b">
          <input
            ref={inputRef}
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="输入命令..."
            className="w-full bg-transparent outline-none text-sm"
          />
        </div>

        {/* Command list */}
        <div className="max-h-[300px] overflow-y-auto">
          {filteredCommands.length === 0 ? (
            <div className="p-4 text-center text-muted-foreground text-sm">
              未找到匹配的命令
            </div>
          ) : (
            filteredCommands.map((cmd, idx) => (
              <div
                key={cmd.name}
                onClick={() => executeCommand(cmd)}
                className={`px-4 py-2 cursor-pointer flex items-center gap-3 ${
                  idx === selectedIndex ? 'bg-accent' : 'hover:bg-accent/50'
                }`}
              >
                <span className={`font-mono text-sm ${categoryColors[cmd.category]}`}>
                  {cmd.name}
                </span>
                {cmd.aliases && (
                  <span className="text-xs text-muted-foreground">
                    ({cmd.aliases.join(', ')})
                  </span>
                )}
                <span className="text-sm text-muted-foreground flex-1">
                  {cmd.description}
                </span>
              </div>
            ))
          )}
        </div>

        {/* Footer hint */}
        <div className="px-4 py-2 bg-muted/30 text-xs text-muted-foreground flex gap-4">
          <span><kbd className="px-1 bg-muted rounded">↑↓</kbd> 导航</span>
          <span><kbd className="px-1 bg-muted rounded">Enter</kbd> 执行</span>
          <span><kbd className="px-1 bg-muted rounded">Tab</kbd> 补全</span>
          <span><kbd className="px-1 bg-muted rounded">Esc</kbd> 关闭</span>
        </div>
      </div>
    </div>
  );
}