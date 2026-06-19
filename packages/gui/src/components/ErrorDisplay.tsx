import React from 'react';
import type { ChatMessage } from '../stores/chatStore';

interface ErrorDisplayProps {
  error: ChatMessage;
  onRetry?: () => void;
  onDismiss?: () => void;
  onCopy?: () => void;
}

// Error type classification
function classifyError(message: string): {
  type: 'api' | 'network' | 'auth' | 'rate_limit' | 'timeout' | 'internal' | 'unknown';
  severity: 'critical' | 'warning' | 'info';
  recoverable: boolean;
} {
  const lowerMessage = message.toLowerCase();

  // API errors
  if (lowerMessage.includes('api key') || lowerMessage.includes('authentication')) {
    return { type: 'auth', severity: 'critical', recoverable: false };
  }
  if (lowerMessage.includes('rate limit') || lowerMessage.includes('quota')) {
    return { type: 'rate_limit', severity: 'warning', recoverable: true };
  }
  if (lowerMessage.includes('timeout') || lowerMessage.includes('timed out')) {
    return { type: 'timeout', severity: 'warning', recoverable: true };
  }

  // Network errors
  if (lowerMessage.includes('network') || lowerMessage.includes('connection') || lowerMessage.includes('fetch')) {
    return { type: 'network', severity: 'warning', recoverable: true };
  }

  // Internal errors
  if (lowerMessage.includes('internal') || lowerMessage.includes('server error')) {
    return { type: 'internal', severity: 'critical', recoverable: true };
  }

  return { type: 'unknown', severity: 'warning', recoverable: true };
}

// Error type icons and colors
const ERROR_TYPE_CONFIG: Record<string, { icon: string; color: string; label: string }> = {
  api: { icon: '📡', color: 'text-blue-500', label: 'API Error' },
  auth: { icon: '🔐', color: 'text-red-500', label: 'Authentication Error' },
  network: { icon: '🌐', color: 'text-orange-500', label: 'Network Error' },
  rate_limit: { icon: '⏱', color: 'text-yellow-500', label: 'Rate Limit' },
  timeout: { icon: '⏳', color: 'text-yellow-500', label: 'Timeout' },
  internal: { icon: '💥', color: 'text-red-500', label: 'Internal Error' },
  unknown: { icon: '❌', color: 'text-gray-500', label: 'Error' },
};

// Severity backgrounds
const SEVERITY_BG: Record<string, string> = {
  critical: 'bg-red-500/10 border-red-500',
  warning: 'bg-yellow-500/10 border-yellow-500',
  info: 'bg-blue-500/10 border-blue-500',
};

export function ErrorDisplay({ error, onRetry, onDismiss, onCopy }: ErrorDisplayProps) {
  const classification = classifyError(error.content);
  const typeConfig = ERROR_TYPE_CONFIG[classification.type];

  // Suggested actions based on error type
  const suggestedActions: string[] = [];
  if (classification.type === 'auth') {
    suggestedActions.push('检查 API Key 配置', '确认 API Key 是否有效');
  } else if (classification.type === 'rate_limit') {
    suggestedActions.push('等待几分钟后重试', '考虑降低请求频率');
  } else if (classification.type === 'network') {
    suggestedActions.push('检查网络连接', '确认防火墙设置');
  } else if (classification.type === 'timeout') {
    suggestedActions.push('尝试简化请求', '检查网络延迟');
  }

  return (
    <div className={`rounded-lg border p-4 ${SEVERITY_BG[classification.severity]}`}>
      {/* Header */}
      <div className="flex items-start gap-3">
        <span className={`text-2xl ${typeConfig.color}`}>
          {typeConfig.icon}
        </span>
        <div className="flex-1">
          <div className="flex items-center gap-2">
            <span className={`font-semibold ${typeConfig.color}`}>
              {typeConfig.label}
            </span>
            <span className={`px-1.5 py-0.5 rounded text-xs ${
              classification.severity === 'critical' ? 'bg-red-500/20 text-red-500' :
              classification.severity === 'warning' ? 'bg-yellow-500/20 text-yellow-500' :
              'bg-blue-500/20 text-blue-500'
            }`}>
              {classification.severity}
            </span>
          </div>

          {/* Error message */}
          <div className="mt-2 text-sm">
            <pre className="whitespace-pre-wrap text-muted-foreground">
              {error.content}
            </pre>
          </div>

          {/* Timestamp */}
          {error.timestamp && (
            <div className="text-xs text-muted-foreground mt-2">
              {new Date(error.timestamp).toLocaleString()}
            </div>
          )}
        </div>
      </div>

      {/* Suggested actions */}
      {suggestedActions.length > 0 && (
        <div className="mt-4 p-3 bg-muted/30 rounded">
          <div className="text-xs font-medium text-muted-foreground mb-2">
            建议操作:
          </div>
          <ul className="space-y-1">
            {suggestedActions.map((action, idx) => (
              <li key={idx} className="text-xs text-muted-foreground flex items-center gap-1">
                <span className={`${typeConfig.color}`}>•</span>
                {action}
              </li>
            ))}
          </ul>
        </div>
      )}

      {/* Action buttons */}
      <div className="flex gap-2 mt-4">
        {/* Retry button (for recoverable errors) */}
        {classification.recoverable && onRetry && (
          <button
            onClick={onRetry}
            className="px-3 py-1.5 bg-primary text-primary-foreground rounded text-sm hover:bg-primary/90 transition-colors flex items-center gap-1.5"
          >
            <span>🔄</span>
            <span>Retry</span>
          </button>
        )}

        {/* Copy button */}
        <button
          onClick={() => {
            navigator.clipboard.writeText(error.content);
            onCopy?.();
          }}
          className="px-3 py-1.5 bg-muted text-muted-foreground rounded text-sm hover:bg-accent transition-colors flex items-center gap-1.5"
        >
          <span>📋</span>
          <span>Copy</span>
        </button>

        {/* Dismiss button */}
        {onDismiss && (
          <button
            onClick={onDismiss}
            className="px-3 py-1.5 bg-muted text-muted-foreground rounded text-sm hover:bg-accent transition-colors flex items-center gap-1.5"
          >
            <span>✕</span>
            <span>Dismiss</span>
          </button>
        )}
      </div>

      {/* Recovery indicator */}
      {classification.recoverable && (
        <div className="text-xs text-muted-foreground mt-2 flex items-center gap-1">
          <span className="text-green-500">✓</span>
          <span>此错误可以恢复</span>
        </div>
      )}
    </div>
  );
}