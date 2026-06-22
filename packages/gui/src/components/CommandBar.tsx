import React, { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useChatStore } from '../stores/chatStore';
import { useConfigStore } from '../stores/configStore';
import { useSessionStore, SessionInfo } from '../stores/sessionStore';
import { useToastContext } from '../contexts/ToastContext';
import { useConfirmDialog, ConfirmDialog } from '../components/shared';
import { useModalFocusTrap } from '../hooks/useModalFocusTrap';

// Type definitions for backend responses
interface ToolInfo {
  name: string;
  description?: string;
}

interface SkillInfo {
  name: string;
  description?: string;
}

interface MemoryEntry {
  name: string;
  type?: string;
}

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
  { name: '/history', aliases: ['/stats'], description: '会话统计信息', category: 'general' },

  // Session commands
  { name: '/new', description: '新建会话', category: 'session' },
  { name: '/save', description: '保存会话', category: 'session' },
  { name: '/sessions', aliases: ['/resume'], description: '显示会话历史', category: 'session' },
  { name: '/load', description: '加载会话 <id>', category: 'session', requiresArg: true },
  { name: '/retry', description: '重试最后消息', category: 'session' },

  // Config commands
  { name: '/mode', description: '切换批准模式 (ask/auto/strict)', category: 'config', requiresArg: false },
  { name: '/model', description: '显示/切换模型', category: 'config' },
  { name: '/config', description: '显示配置信息', category: 'config' },

  // Tools commands
  { name: '/tools', description: '列出可用工具', category: 'tools' },
  { name: '/skills', description: '列出已加载技能', category: 'tools' },
  { name: '/compact', aliases: ['/compress'], description: '压缩上下文', category: 'tools' },
  { name: '/memory', description: '查看/管理记忆', category: 'tools' },
  { name: '/init', description: '初始化项目配置', category: 'tools' },
  { name: '/overview', description: '查看项目概览', category: 'tools' },

  // Debug commands
  { name: '/debug', description: '切换调试模式', category: 'debug' },
  { name: '/workflow', aliases: ['/wf'], description: '工作流管理', category: 'debug' },
  { name: '/mcp', description: 'MCP服务器状态', category: 'debug' },
  { name: '/stats', aliases: ['/token'], description: 'Token统计', category: 'debug' },
  { name: '/lsp', description: 'LSP服务器状态', category: 'debug' },
  { name: '/codegraph', aliases: ['/cg'], description: 'CodeGraph状态', category: 'debug' },
  { name: '/loop', description: '创建循环任务', category: 'debug' },
  { name: '/cron', description: '管理定时任务', category: 'debug' },
];

interface CommandBarProps {
  onSubmitCommand: (command: string) => void;
  onClose: () => void;
  onShowLoopDialog?: () => void;  // Optional callback for loop dialog
  onShowCronDialog?: () => void;  // Optional callback for cron dialog
  onShowSessionSwitcher?: () => void;  // Optional callback for session switcher
  onShowMemoryPanel?: () => void;  // Optional callback for memory panel
  onShowToolsSkillsPanel?: () => void;  // Optional callback for tools/skills panel
}

export function CommandBar({
  onSubmitCommand,
  onClose,
  onShowLoopDialog,
  onShowCronDialog,
  onShowSessionSwitcher,
  onShowMemoryPanel,
  onShowToolsSkillsPanel
}: CommandBarProps) {
  const [input, setInput] = useState('/');
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [filteredCommands, setFilteredCommands] = useState<Command[]>(COMMANDS);
  const inputRef = useRef<HTMLInputElement>(null);
  const modalRef = useRef<HTMLDivElement>(null);

  const clearMessages = useChatStore((s) => s.clearMessages);
  const toggleDebugPanel = useChatStore((s) => s.toggleDebugPanel);
  const toggleWorkflowPanel = useChatStore((s) => s.toggleWorkflowPanel);
  const retryLastMessage = useChatStore((s) => s.retryLastMessage);
  const config = useConfigStore((s) => s.config);
  const updateConfig = useConfigStore((s) => s.updateConfig);
  const toast = useToastContext();

  const { visible: showConfirm, config: confirmConfig, showConfirm: openConfirm, handleConfirm, handleCancel } = useConfirmDialog();

  // Use shared focus trap hook
  useModalFocusTrap(modalRef, onClose, { autoFocus: true, onEscape: true });

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
        openConfirm(
          'Clear Messages',
          'Are you sure you want to clear all messages? This action cannot be undone.',
          () => {
            clearMessages();
            toast.addToast({ type: 'success', message: 'Messages cleared' });
          },
          'warning'
        );
        break;
      case '/debug':
        toggleDebugPanel();
        break;
      case '/workflow':
        toggleWorkflowPanel();
        break;
      case '/retry':
        try {
          await retryLastMessage();
          toast.addToast({ type: 'info', message: '正在重试最后消息' });
        } catch (e) {
          toast.addToast({ type: 'error', message: '重试失败: ' + (e instanceof Error ? e.message : '未知错误') });
        }
        break;
      case '/new':
        createSession();
        clearMessages();
        toast.addToast({ type: 'success', message: '已创建新会话' });
        break;
      case '/history':
      case '/stats': {
        // Show session statistics (matching TUI /history)
        const messages = useChatStore.getState().messages;
        const userCount = messages.filter(m => m.role === 'user').length;
        const assistantCount = messages.filter(m => m.role === 'assistant').length;
        const toolCount = messages.filter(m => m.role === 'tool').length;
        const pendingCount = useChatStore.getState().pendingMessages.length;
        const sessionTotalOut = useChatStore.getState().outputTokens;
        toast.addToast({
          type: 'info',
          message: `📊 Session: ${userCount} user, ${assistantCount} assistant, ${toolCount} tools, ${pendingCount} queued, ${sessionTotalOut} output tokens`
        });
        console.log(`Session stats: ${userCount} user, ${assistantCount} assistant, ${toolCount} tools, ${pendingCount} queued`);
        break;
      }
      case '/mode': {
        // Cycle through modes: auto -> ask -> strict -> auto
        const modes = ['auto', 'ask', 'strict'];
        const currentMode = config?.approve_mode || 'ask';
        const currentIdx = modes.indexOf(currentMode);
        const nextMode = modes[(currentIdx + 1) % modes.length];
        try {
          // Update config (this will call backend and refresh local state)
          await updateConfig({ approve_mode: nextMode });
          toast.addToast({
            type: 'success',
            message: `批准模式已切换: ${currentMode} → ${nextMode}`
          });
          console.log(`Switching mode from ${currentMode} to ${nextMode}`);
        } catch (e) {
          toast.addToast({
            type: 'error',
            message: '切换模式失败: ' + (e instanceof Error ? e.message : '未知错误')
          });
        }
        break;
      }
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
        // Show session switcher dialog
        if (onShowSessionSwitcher) {
          onShowSessionSwitcher();
        } else {
          // Fallback: show session list info
          try {
            const sessions = await invoke<SessionInfo[]>('list_sessions');
            const count = sessions?.length || 0;
            toast.addToast({
              type: 'info',
              message: `找到 ${count} 个会话，使用 Ctrl+S 打开会话切换器`
            });
            console.log('Session history:', sessions);
          } catch (e) {
            toast.addToast({ type: 'error', message: '加载会话列表失败' });
          }
        }
        break;
      case '/save':
        // Save current session
        try {
          await invoke('save_session');
          toast.addToast({ type: 'success', message: '✓ 会话已保存' });
        } catch (e) {
          toast.addToast({ type: 'error', message: '保存会话失败' });
        }
        break;
      case '/init':
        // Initialize project configuration
        try {
          await invoke('init_project');
          toast.addToast({ type: 'info', message: '🔄 正在生成项目概览...' });
        } catch (e) {
          toast.addToast({ type: 'error', message: '项目初始化失败' });
        }
        break;
      case '/overview':
        // Show project overview
        try {
          await invoke('show_overview');
          toast.addToast({ type: 'info', message: '正在加载项目概览...' });
        } catch (e) {
          toast.addToast({ type: 'error', message: '加载概览失败' });
        }
        break;
      case '/load':
        // Load session by ID (requires argument)
        toast.addToast({ type: 'info', message: '加载会话功能需要后端支持' });
        console.log('Load session - requires argument');
        break;
      case '/loop':
        // Show loop task dialog
        if (onShowLoopDialog) {
          onShowLoopDialog();
        } else {
          toast.addToast({ type: 'info', message: '创建循环任务对话框' });
        }
        break;
      case '/cron':
        // Show cron task management dialog
        if (onShowCronDialog) {
          onShowCronDialog();
        } else {
          toast.addToast({ type: 'info', message: '定时任务管理对话框' });
        }
        break;
      case '/lsp':
        // Show LSP panel (handled by parent)
        break;
      case '/codegraph':
      case '/cg':
        // Show CodeGraph panel (handled by parent)
        break;
      case '/mcp':
        // Show MCP panel (handled by parent)
        break;
      case '/tools':
        // Show tools/skills list
        if (onShowToolsSkillsPanel) {
          onShowToolsSkillsPanel();
        } else {
          try {
            const tools = await invoke<ToolInfo[]>('list_tools');
            const skills = await invoke<SkillInfo[]>('list_skills');
            const toolCount = tools?.length || 0;
            const skillCount = skills?.length || 0;
            toast.addToast({
              type: 'info',
              message: `可用工具: ${toolCount} 个，已加载技能: ${skillCount} 个`
            });
            console.log('Tools:', tools, 'Skills:', skills);
          } catch (e) {
            toast.addToast({ type: 'error', message: '加载工具列表失败' });
          }
        }
        break;
      case '/skills':
        // Show skills list
        try {
          const skills = await invoke<SkillInfo[]>('list_skills');
          if (Array.isArray(skills) && skills.length > 0) {
            const skillNames = skills.map(s => s.name || s).join(', ');
            toast.addToast({
              type: 'info',
              message: `已加载技能 (${skills.length}): ${skillNames}`
            });
          } else {
            toast.addToast({ type: 'info', message: '无已加载技能' });
          }
          console.log('Skills list:', skills);
        } catch (e) {
          toast.addToast({
            type: 'error',
            message: '加载技能失败: ' + (e instanceof Error ? e.message : '未知错误')
          });
        }
        break;
      case '/compact':
      case '/compress':
        // Trigger context compression
        try {
          await invoke('compress_context');
          toast.addToast({ type: 'success', message: '上下文压缩已触发' });
        } catch (e) {
          toast.addToast({ type: 'error', message: '压缩失败' });
        }
        break;
      case '/memory':
        // Show memory panel (handled by parent)
        if (onShowMemoryPanel) {
          onShowMemoryPanel();
        } else {
          try {
            const memories = await invoke<MemoryEntry[]>('list_memories');
            const count = memories?.length || 0;
            toast.addToast({
              type: 'info',
              message: `已积累 ${count} 条记忆，使用 Ctrl+M 打开记忆面板`
            });
            console.log('Memories:', memories);
          } catch (e) {
            toast.addToast({ type: 'error', message: '加载记忆失败' });
          }
        }
        break;
      case '/exit':
      case '/quit':
        openConfirm(
          'Exit Program',
          'Are you sure you want to exit MatrixCode?',
          () => {
            toast.addToast({ type: 'warning', message: 'Exit program (requires backend support)' });
            console.log('Exit program confirmed');
          },
          'danger'
        );
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

  // Render confirm dialog
  return (
    <>
      <div
        ref={modalRef}
        className="fixed inset-0 bg-black/50 flex items-start justify-center z-50 p-4 pt-[20%]"
        role="dialog"
        aria-modal="true"
        aria-labelledby="command-bar-title"
        onClick={(e) => {
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
              aria-label="Command input"
              className="w-full bg-transparent outline-none text-sm"
            />
          </div>

          {/* Command list */}
          <div className="max-h-[300px] overflow-y-auto" role="listbox" aria-label="命令列表">
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
                  role="option"
                  aria-selected={idx === selectedIndex}
                  aria-label={`${cmd.name}: ${cmd.description}`}
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

      {/* Confirmation dialog */}
      {showConfirm && confirmConfig && (
        <ConfirmDialog
          title={confirmConfig.title}
          message={confirmConfig.message}
          onConfirm={handleConfirm}
          onCancel={handleCancel}
          confirmText="Confirm"
          cancelText="Cancel"
          variant={confirmConfig.variant || 'warning'}
        />
      )}
    </>
  );
}