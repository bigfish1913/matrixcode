import React, { useState, useEffect } from 'react';
import { useChatStore } from '../stores/chatStore';
import { useConfigStore } from '../stores/configStore';
import { useApprovalStore } from '../stores/approvalStore';
import { useMcpStatus, useLspStatus, useCodeGraphStatus } from '../contexts/ServerStatusContext';
import { TodoIndicator } from './TodoIndicator';
import { formatTokenCount, formatElapsed } from '../utils/formatters';

// Version constant (should be imported from package.json in real app)
const VERSION = '0.4.48';

// Server status indicator (matching TUI mcp/lsp/codegraph status)
interface ServerStatusProps {
  name: string;
  status: 'connected' | 'disconnected' | 'initializing' | 'error' | 'disabled' | 'configured';
  icon?: string;
  count?: number;  // Show count (tool count for MCP, server count for LSP, pending count for CG)
}

function ServerStatus({ name, status, icon, count }: ServerStatusProps) {
  const statusConfig = {
    connected: { color: 'text-green-500', icon: '●' },
    disconnected: { color: 'text-gray-400', icon: '○' },
    initializing: { color: 'text-yellow-500 animate-pulse', icon: '◐' },
    error: { color: 'text-red-500', icon: '✗' },
    disabled: { color: 'text-gray-300', icon: '◌' },
    configured: { color: 'text-yellow-500', icon: '◐' },  // Configured but not started (yellow warning)
  };

  const config = statusConfig[status];

  // Format display text with count (matching TUI: ●MCP(N), ●LSP(N), ●CG(N))
  const displayText = count && count > 0 ? `${name}(${count})` : name;

  return (
    <span className={`flex items-center gap-1 ${config.color}`}>
      {icon && <span className="text-xs">{icon}</span>}
      <span className="text-xs">{config.icon}</span>
      <span className="text-xs font-medium">{displayText}</span>
    </span>
  );
}

// Approve mode indicator with color and tooltip (matching TUI approve_mode display)
function ApproveModeIndicator({ mode }: { mode?: string }) {
  if (!mode) return null;

  const modeConfig = {
    ask: { color: 'bg-gray-400/20 text-gray-500', label: 'ASK', tooltip: '每次操作都需要确认' },
    auto: { color: 'bg-green-500/20 text-green-600', label: 'AUTO', tooltip: '自动执行，仅危险操作需确认' },
    strict: { color: 'bg-red-500/20 text-red-600', label: 'STRICT', tooltip: '严格模式，所有操作自动执行（包括危险操作）' },
  };

  const config = modeConfig[mode as keyof typeof modeConfig] || modeConfig.auto;

  return (
    <span
      className={`px-1.5 py-0.5 rounded text-xs font-mono ${config.color} cursor-help`}
      title={config.tooltip}
      aria-label={`批准模式: ${config.label} - ${config.tooltip}`}
    >
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

interface StatusBarProps {
  onOpenModelSwitcher?: () => void;
  onOpenSettings?: () => void;  // Open settings panel (matching VSCode matrixcode.openSettings)
  onOpenMcpPanel?: () => void;
  onOpenLspPanel?: () => void;
  onOpenCodeGraphPanel?: () => void;
  onOpenSearchPanel?: () => void;  // Open search panel for messages
}

export function StatusBar({ onOpenModelSwitcher, onOpenSettings, onOpenMcpPanel, onOpenLspPanel, onOpenCodeGraphPanel, onOpenSearchPanel }: StatusBarProps) {
  const status = useChatStore((s) => s.status);
  const activity = useChatStore((s) => s.activity);
  const inputTokens = useChatStore((s) => s.inputTokens);
  const outputTokens = useChatStore((s) => s.outputTokens);
  const cacheReadTokens = useChatStore((s) => s.cacheReadTokens);
  const cacheCreationTokens = useChatStore((s) => s.cacheCreationTokens);
  const pendingMessages = useChatStore((s) => s.pendingMessages);

  const config = useConfigStore((s) => s.config);
  const approveMode = config?.approve_mode || 'auto';

  // Get approval queue status
  const pendingApprovals = useApprovalStore((s) => s.pendingApprovals);

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
  const formatElapsedTime = (seconds: number): string => {
    return formatElapsed(seconds);
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
    <div className="border-b bg-card px-4 py-2 flex items-center justify-between text-xs" role="status" aria-live="polite">
      {/* Left section: Status and Activity */}
      <div className="flex items-center gap-3">
        {/* Status indicator */}
        <div className="flex items-center gap-1.5">
          <span className={`${status === 'running' ? 'text-blue-500 animate-pulse' : 'text-green-500'}`} aria-hidden="true">
            {status === 'running' ? '●' : '●'}
          </span>
          <span className="text-muted-foreground" aria-label={status === 'running' ? 'Agent is processing' : 'Agent is ready'}>
            {status === 'running' ? 'Processing' : 'Ready'}
          </span>
        </div>

        {/* Activity indicator */}
        {activity.type !== 'idle' && (
          <div className="flex items-center gap-1.5 animate-fade-in" aria-label={`Agent activity: ${activityInfo.label}`}>
            <span className={activityInfo.color} aria-hidden="true">{activityInfo.icon}</span>
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
                {formatElapsedTime(elapsedSeconds)}
              </span>
            )}
          </div>
        )}

        {/* Todo progress indicator */}
        <TodoIndicator />

        {/* Pending messages indicator */}
        {pendingMessages.length > 0 && (
          <div className="flex items-center gap-1 px-1.5 py-0.5 bg-yellow-500/10 rounded text-yellow-600" aria-label={`${pendingMessages.length} messages pending`}>
            <span aria-hidden="true">⏳</span>
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

      {/* Right section: Mode, Model, Version */}
      <div className="flex items-center gap-3">
        {/* Version display (matching TUI status bar when width >= 90) */}
        <span
          className="text-xs text-muted-foreground font-mono cursor-default"
          title={`MatrixCode GUI v${VERSION}`}
          aria-label={`版本: ${VERSION}`}
        >
          v{VERSION}
        </span>

        {/* Approve mode with pending queue indicator */}
        <div className="flex items-center gap-1">
          <ApproveModeIndicator mode={approveMode} />
          {pendingApprovals.length > 0 && (
            <span className="px-1.5 py-0.5 bg-yellow-500/20 text-yellow-600 rounded text-xs font-mono animate-pulse">
              {pendingApprovals.length}
            </span>
          )}
        </div>

        {/* Model name (clickable - opens settings to change model, matching VSCode) */}
        <button
          onClick={onOpenSettings || onOpenModelSwitcher}
          className="flex items-center gap-1 text-muted-foreground hover:text-foreground transition-colors cursor-pointer"
          title="点击打开设置（可切换模型）"
          aria-label={`Current model: ${config?.model || 'claude'}. Click to change model.`}
        >
          <span aria-hidden="true">🤖</span>
          <span className="font-mono">
            {config?.model?.split('-').slice(0, 2).join('-') || 'claude'}
          </span>
        </button>

        {/* Search button */}
        <button
          onClick={onOpenSearchPanel}
          className="flex items-center gap-1 text-muted-foreground hover:text-foreground transition-colors cursor-pointer"
          title="搜索消息 (Ctrl+F)"
          aria-label="搜索消息"
        >
          <span aria-hidden="true">🔍</span>
        </button>

        {/* MCP/LSP/CodeGraph status (from ServerStatusContext) - clickable */}
        <div className="flex items-center gap-2">
          {/* MCP status - show tool count and configured-but-not-started warning */}
          {(() => {
            // Calculate total tool count from started servers (matching TUI)
            const mcpToolCount = mcpStatus.servers
              .filter(s => s.status === 'running' || s.status === 'connected')
              .reduce((sum, s) => sum + (s.tools?.length || 0), 0);

            // Determine status (matching TUI logic)
            const mcpHasConfigured = mcpStatus.servers.length > 0;
            const mcpHasStarted = mcpStatus.servers.some(s => s.status === 'running' || s.status === 'connected');
            const mcpStatusType = mcpHasStarted ? 'connected' : mcpHasConfigured ? 'configured' : 'disconnected';

            return (
              <button
                onClick={() => onOpenMcpPanel?.()}
                className="flex items-center gap-1 hover:opacity-80 transition-opacity cursor-pointer disabled:cursor-not-allowed disabled:opacity-50"
                disabled={!onOpenMcpPanel}
                title={onOpenMcpPanel ? "点击查看 MCP 详情" : "MCP 详情不可用"}
                aria-label={`MCP server status: ${mcpStatusType}. ${onOpenMcpPanel ? 'Click to view details.' : 'Details not available.'}`}
              >
                <ServerStatus
                  name="MCP"
                  status={mcpStatusType}
                  icon="🔌"
                  count={mcpToolCount}
                />
              </button>
            );
          })()}

          {/* LSP status - show connected server count */}
          {(() => {
            // Calculate connected server count (matching TUI)
            const lspConnectedCount = lspStatus.servers
              .filter(s => s.status === 'connected' || s.status === 'running')
              .length;

            // Check for starting/error status (matching TUI priority)
            const lspStarting = lspStatus.servers.some(s => s.status === 'starting' || s.status === 'initializing');
            const lspError = lspStatus.servers.some(s => s.status === 'error');

            const lspStatusType = lspError ? 'error' : lspStarting ? 'initializing' : lspConnectedCount > 0 ? 'connected' : lspStatus.servers.length > 0 ? 'configured' : 'disconnected';

            // Only show count if > 1 (matching TUI)
            const showCount = lspConnectedCount > 1 ? lspConnectedCount : undefined;

            return (
              <button
                onClick={() => onOpenLspPanel?.()}
                className="flex items-center gap-1 hover:opacity-80 transition-opacity cursor-pointer disabled:cursor-not-allowed disabled:opacity-50"
                disabled={!onOpenLspPanel}
                title={onOpenLspPanel ? "点击查看 LSP 详情" : "LSP 详情不可用"}
                aria-label={`LSP server status: ${lspStatusType}. ${onOpenLspPanel ? 'Click to view details.' : 'Details not available.'}`}
              >
                <ServerStatus
                  name="LSP"
                  status={lspStatusType}
                  icon="📝"
                  count={showCount}
                />
              </button>
            );
          })()}

          {/* CodeGraph status - show pending files count */}
          {(() => {
            // Show pending files count (matching TUI)
            const cgPendingCount = codegraphStatus.pendingFiles;

            // Determine status (matching TUI logic)
            const cgStatusType = cgPendingCount > 0 ? 'connected' : codegraphStatus.indexing ? 'initializing' : codegraphStatus.initialized ? 'connected' : 'disconnected';

            // Yellow warning when pending > 0 (matching TUI)
            const cgColorOverride = cgPendingCount > 0 ? 'configured' : cgStatusType;

            return (
              <button
                onClick={() => onOpenCodeGraphPanel?.()}
                className="flex items-center gap-1 hover:opacity-80 transition-opacity cursor-pointer disabled:cursor-not-allowed disabled:opacity-50"
                disabled={!onOpenCodeGraphPanel}
                title={onOpenCodeGraphPanel ? "点击查看 CodeGraph 详情" : "CodeGraph 详情不可用"}
                aria-label={`CodeGraph status: ${cgStatusType}. ${onOpenCodeGraphPanel ? 'Click to view details.' : 'Details not available.'}`}
              >
                <ServerStatus
                  name="CG"
                  status={cgPendingCount > 0 ? 'configured' : cgStatusType}
                  icon="📊"
                  count={cgPendingCount}
                />
              </button>
            );
          })()}
        </div>
      </div>
    </div>
  );
}