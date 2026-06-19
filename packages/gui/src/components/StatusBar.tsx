import React, { useState, useEffect } from 'react';
import { useChatStore } from '../stores/chatStore';
import { useConfigStore } from '../stores/configStore';
import { useMcpStatus, useLspStatus, useCodeGraphStatus } from '../contexts/ServerStatusContext';
import { TodoIndicator } from './TodoIndicator';

// Token format helper
function formatTokenCount(count: number): string {
  if (count >= 1000) return `${(count / 1000).toFixed(1)}k`;
  return String(count);
}

// Server status indicator (matching TUI mcp/lsp/codegraph status)
interface ServerStatusProps {
  name: string;
  status: 'connected' | 'disconnected' | 'initializing' | 'error' | 'disabled';
  icon?: string;
}

function ServerStatus({ name, status, icon }: ServerStatusProps) {
  const statusConfig = {
    connected: { color: 'text-green-500', icon: '●' },
    disconnected: { color: 'text-gray-400', icon: '○' },
    initializing: { color: 'text-yellow-500 animate-pulse', icon: '◐' },
    error: { color: 'text-red-500', icon: '✗' },
    disabled: { color: 'text-gray-300', icon: '◌' },
  };

  const config = statusConfig[status];

  return (
    <span className={`flex items-center gap-1 ${config.color}`}>
      {icon && <span className="text-xs">{icon}</span>}
      <span className="text-xs">{config.icon}</span>
      <span className="text-xs font-medium">{name}</span>
    </span>
  );
}

// Approve mode indicator with color (matching TUI approve_mode display)
function ApproveModeIndicator({ mode }: { mode?: string }) {
  if (!mode) return null;

  const modeConfig = {
    ask: { color: 'bg-gray-400/20 text-gray-500', label: 'ASK' },
    auto: { color: 'bg-green-500/20 text-green-600', label: 'AUTO' },
    strict: { color: 'bg-red-500/20 text-red-600', label: 'STRICT' },
  };

  const config = modeConfig[mode as keyof typeof modeConfig] || modeConfig.auto;

  return (
    <span className={`px-1.5 py-0.5 rounded text-xs font-mono ${config.color}`}>
      {config.label}
    </span>
  );
}

// Token usage bar (matching TUI token display)
function TokenUsageBar({
  input,
  output,
  cacheRead,
  cacheCreated,
}: {
  input: number;
  output: number;
  cacheRead?: number;
  cacheCreated?: number;
}) {
  const total = input + output;
  const ratio = total > 0 ? output / total : 0;

  return (
    <div className="flex items-center gap-2 px-2">
      {/* Input tokens */}
      <div className="flex items-center gap-1">
        <span className="text-xs text-muted-foreground">In:</span>
        <span className="text-xs font-mono text-blue-500">{formatTokenCount(input)}</span>
      </div>

      {/* Progress bar */}
      <div className="w-16 h-1.5 bg-muted rounded-full overflow-hidden">
        <div
          className="h-full bg-primary rounded-full transition-all duration-300"
          style={{ width: `${ratio * 100}%` }}
        />
      </div>

      {/* Output tokens */}
      <div className="flex items-center gap-1">
        <span className="text-xs text-muted-foreground">Out:</span>
        <span className="text-xs font-mono text-green-500">{formatTokenCount(output)}</span>
      </div>

      {/* Cache indicators */}
      {cacheRead && cacheRead > 0 && (
        <span className="text-xs text-green-500 flex items-center gap-0.5">
          <span>⚡</span>
          <span className="font-mono">{formatTokenCount(cacheRead)}</span>
        </span>
      )}

      {cacheCreated && cacheCreated > 0 && (
        <span className="text-xs text-blue-500 flex items-center gap-0.5">
          <span>💾</span>
          <span className="font-mono">{formatTokenCount(cacheCreated)}</span>
        </span>
      )}
    </div>
  );
}

export function StatusBar() {
  const status = useChatStore((s) => s.status);
  const activity = useChatStore((s) => s.activity);
  const inputTokens = useChatStore((s) => s.inputTokens);
  const outputTokens = useChatStore((s) => s.outputTokens);
  const cacheReadTokens = useChatStore((s) => s.cacheReadTokens);
  const cacheCreationTokens = useChatStore((s) => s.cacheCreationTokens);
  const pendingMessages = useChatStore((s) => s.pendingMessages);

  const config = useConfigStore((s) => s.config);
  const approveMode = config?.approve_mode || 'auto';

  // Get server status from context
  const mcpStatus = useMcpStatus();
  const lspStatus = useLspStatus();
  const codegraphStatus = useCodeGraphStatus();

  const [elapsedSeconds, setElapsedSeconds] = useState(0);

  // Track elapsed time for activity
  useEffect(() => {
    if (activity.startTime && activity.type !== 'idle') {
      const interval = setInterval(() => {
        setElapsedSeconds((Date.now() - activity.startTime!) / 1000);
      }, 100);
      return () => clearInterval(interval);
    } else {
      setElapsedSeconds(0);
    }
  }, [activity.startTime, activity.type]);

  // Format elapsed time
  const formatElapsed = (seconds: number): string => {
    if (seconds < 60) return `${Math.floor(seconds)}s`;
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs.toString().padStart(2, '0')}`;
  };

  // Activity icon and label (matching TUI Activity::label())
  const activityConfig = {
    idle: { icon: '⏸', label: '就绪', color: 'text-green-500' },
    thinking: { icon: '💭', label: '思考中', color: 'text-purple-500' },
    reading: { icon: '📖', label: '读取', color: 'text-cyan-500' },
    writing: { icon: '📝', label: '写入', color: 'text-yellow-500' },
    editing: { icon: '✏️', label: '编辑', color: 'text-yellow-500' },
    searching: { icon: '🔍', label: '搜索', color: 'text-cyan-500' },
    running: { icon: '⚡', label: '执行', color: 'text-red-500' },
    websearch: { icon: '🌐', label: '网络搜索', color: 'text-blue-500' },
    webfetch: { icon: '⬇️', label: '网络获取', color: 'text-blue-500' },
    tool: { icon: '🔧', label: '工具', color: 'text-cyan-500' },
    asking: { icon: '❓', label: '等待响应', color: 'text-red-500' },
  };

  const activityInfo = activityConfig[activity.type] || activityConfig.idle;

  return (
    <div className="border-b bg-card px-4 py-2 flex items-center justify-between text-xs">
      {/* Left section: Status and Activity */}
      <div className="flex items-center gap-3">
        {/* Status indicator */}
        <div className="flex items-center gap-1.5">
          <span className={`${status === 'running' ? 'text-blue-500 animate-pulse' : 'text-green-500'}`}>
            {status === 'running' ? '●' : '●'}
          </span>
          <span className="text-muted-foreground">
            {status === 'running' ? 'Processing' : 'Ready'}
          </span>
        </div>

        {/* Activity indicator */}
        {activity.type !== 'idle' && (
          <div className="flex items-center gap-1.5 animate-fade-in">
            <span className={activityInfo.color}>{activityInfo.icon}</span>
            <span className={`font-medium ${activityInfo.color}`}>
              {activityInfo.label}
            </span>
            {activity.detail && (
              <span className="text-muted-foreground">
                {activity.detail}
              </span>
            )}
            {elapsedSeconds > 0 && (
              <span className="text-muted-foreground font-mono ml-1">
                {formatElapsed(elapsedSeconds)}
              </span>
            )}
          </div>
        )}

        {/* Todo progress indicator */}
        <TodoIndicator />

        {/* Pending messages indicator */}
        {pendingMessages.length > 0 && (
          <div className="flex items-center gap-1 px-1.5 py-0.5 bg-yellow-500/10 rounded text-yellow-600">
            <span>⏳</span>
            <span>{pendingMessages.length} pending</span>
          </div>
        )}
      </div>

      {/* Middle section: Token usage */}
      {(inputTokens > 0 || outputTokens > 0) && (
        <TokenUsageBar
          input={inputTokens}
          output={outputTokens}
          cacheRead={cacheReadTokens}
          cacheCreated={cacheCreationTokens}
        />
      )}

      {/* Right section: Mode and Model */}
      <div className="flex items-center gap-3">
        {/* Approve mode */}
        <ApproveModeIndicator mode={approveMode} />

        {/* Model name */}
        <div className="flex items-center gap-1 text-muted-foreground">
          <span>🤖</span>
          <span className="font-mono">
            {config?.model?.split('-').slice(0, 2).join('-') || 'claude'}
          </span>
        </div>

        {/* MCP/LSP/CodeGraph status (from ServerStatusContext) */}
        <div className="flex items-center gap-2">
          <ServerStatus
            name="MCP"
            status={mcpStatus.connected ? 'connected' : 'disconnected'}
            icon="🔌"
          />
          <ServerStatus
            name="LSP"
            status={lspStatus.connected ? 'connected' : 'disconnected'}
            icon="📝"
          />
          <ServerStatus
            name="CG"
            status={codegraphStatus.initialized ? 'connected' : codegraphStatus.indexing ? 'initializing' : 'disconnected'}
            icon="📊"
          />
        </div>
      </div>
    </div>
  );
}