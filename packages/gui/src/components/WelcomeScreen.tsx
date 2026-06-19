import React from 'react';
import { useConfigStore } from '../stores/configStore';

// ASCII Art for MatrixCode (adapted from TUI welcome)
const MATRIX_ASCII_ART = `
╔═══════════════════════════════════════════╗
║  ██████╗ █████╗ ███╗   ███╗               ║
║ ██╔════╝██╔══██╗████╗ ████║               ║
║ ██║     ███████║██╔████╔██║               ║
║ ██║     ██╔══██║██║╚██╔╝██║               ║
║ ╚██████╗██║  ██║██║ ╚═╝ ██║    C O D E    ║
║  ╚═════╝╚═╝  ╚═╝╚═╝     ╚═╝               ║
╚═══════════════════════════════════════════╝
`;

interface WelcomeScreenProps {
  onCommandBar: () => void;
  onNewChat: () => void;
  onSettings: () => void;
}

export function WelcomeScreen({ onCommandBar, onNewChat, onSettings }: WelcomeScreenProps) {
  const config = useConfigStore((s) => s.config);
  const projectPath = useConfigStore((s) => s.projectPath);

  // Quick action buttons
  const quickActions = [
    { icon: '💬', label: '开始对话', description: '输入消息开始与 AI 对话', action: () => {} },
    { icon: '📁', label: '打开项目', description: '选择一个项目目录', action: onSettings },
    { icon: '⚙️', label: '配置设置', description: '设置 API Key 和模型', action: onSettings },
    { icon: '⌨️', label: '斜杠命令', description: '浏览可用命令', action: onCommandBar },
  ];

  // Features list
  const features = [
    { icon: '🧠', title: '智能代码助手', desc: '多模型 AI 辅助编程' },
    { icon: '📝', title: '文件操作', desc: '读取、编辑、创建文件' },
    { icon: '🔍', title: '智能搜索', desc: 'Grep、Glob、Web 搜索' },
    { icon: '🔧', title: '工具系统', desc: '可扩展的工具集' },
    { icon: '📚', title: '技能系统', desc: '自定义技能脚本' },
    { icon: '💾', title: '记忆系统', desc: '跨会话持久化记忆' },
    { icon: '🌐', title: 'MCP 支持', desc: 'Model Context Protocol' },
    { icon: '📊', title: 'CodeGraph', desc: '代码知识图谱' },
  ];

  return (
    <div className="flex items-center justify-center h-full">
      <div className="max-w-2xl w-full px-6 py-8 text-center space-y-6">
        {/* ASCII Art Title */}
        <div className="space-y-2">
          <pre className="text-primary font-mono text-xs leading-tight hidden md:block">
            {MATRIX_ASCII_ART}
          </pre>
          {/* Fallback for small screens */}
          <div className="md:hidden text-3xl font-bold text-primary">
            ⚡ MatrixCode
          </div>
          <p className="text-muted-foreground text-sm">
            AI-powered intelligent code assistant
          </p>
        </div>

        {/* Status info */}
        <div className="flex items-center justify-center gap-4 text-xs">
          {config?.model && (
            <span className="px-2 py-1 bg-muted rounded-full font-mono">
              {config.model}
            </span>
          )}
          {config?.approve_mode && (
            <span className="px-2 py-1 bg-muted rounded-full">
              Mode: {config.approve_mode.toUpperCase()}
            </span>
          )}
          {projectPath && (
            <span className="px-2 py-1 bg-muted rounded-full truncate max-w-[200px]">
              📁 {projectPath}
            </span>
          )}
        </div>

        {/* Quick actions */}
        <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
          {quickActions.map((action) => (
            <button
              key={action.label}
              onClick={action.action}
              className="p-4 bg-card border rounded-lg hover:bg-accent transition-colors text-left"
            >
              <div className="text-2xl mb-2">{action.icon}</div>
              <div className="font-medium text-sm">{action.label}</div>
              <div className="text-xs text-muted-foreground mt-1">
                {action.description}
              </div>
            </button>
          ))}
        </div>

        {/* Features grid */}
        <div className="grid grid-cols-2 md:grid-cols-4 gap-2 text-xs">
          {features.map((feature) => (
            <div key={feature.title} className="p-2 bg-muted/30 rounded flex items-center gap-2">
              <span>{feature.icon}</span>
              <div>
                <div className="font-medium">{feature.title}</div>
                <div className="text-muted-foreground">{feature.desc}</div>
              </div>
            </div>
          ))}
        </div>

        {/* Keyboard shortcuts hint */}
        <div className="space-y-2">
          <div className="text-xs text-muted-foreground">快捷键</div>
          <div className="flex flex-wrap gap-2 justify-center text-xs">
            <div className="flex gap-1 items-center">
              <kbd className="px-1.5 py-0.5 bg-muted border rounded font-mono">Enter</kbd>
              <span className="text-muted-foreground">发送</span>
            </div>
            <div className="flex gap-1 items-center">
              <kbd className="px-1.5 py-0.5 bg-muted border rounded font-mono">Shift+Enter</kbd>
              <span className="text-muted-foreground">换行</span>
            </div>
            <div className="flex gap-1 items-center">
              <kbd className="px-1.5 py-0.5 bg-muted border rounded font-mono">↑↓</kbd>
              <span className="text-muted-foreground">历史</span>
            </div>
            <div className="flex gap-1 items-center">
              <kbd className="px-1.5 py-0.5 bg-muted border rounded font-mono">Ctrl+D</kbd>
              <span className="text-muted-foreground">调试</span>
            </div>
            <div className="flex gap-1 items-center">
              <kbd className="px-1.5 py-0.5 bg-muted border rounded font-mono">Alt+W</kbd>
              <span className="text-muted-foreground">工作流</span>
            </div>
            <div className="flex gap-1 items-center">
              <kbd className="px-1.5 py-0.5 bg-muted border rounded font-mono">/</kbd>
              <span className="text-muted-foreground">命令</span>
            </div>
          </div>
        </div>

        {/* Version info */}
        <div className="text-xs text-muted-foreground/50">
          Version 0.4.48 • Powered by Claude
        </div>
      </div>
    </div>
  );
}