import React from 'react';

interface QueueIndicatorProps {
  messages: Array<{ content: string; timestamp: number }>;
  onDismiss?: () => void;
}

export function QueueIndicator({ messages, onDismiss }: QueueIndicatorProps) {
  if (messages.length === 0) {
    return null;
  }

  const count = messages.length;
  const preview = messages.map(m => m.content).join(' • ');

  return (
    <div className="px-4 py-2 bg-cyan-500/10 border-b border-cyan-500/20 text-sm">
      <div className="flex items-center gap-2">
        {/* Icon */}
        <span className="text-cyan-500 animate-pulse">⏳</span>

        {/* Message count */}
        <span className="font-medium text-cyan-600">
          {count} pending message{count > 1 ? 's' : ''}
        </span>

        {/* Preview */}
        <span className="text-muted-foreground truncate max-w-[400px] text-xs">
          {preview.slice(0, 100)}{preview.length > 100 ? '...' : ''}
        </span>

        {/* Dismiss button */}
        {onDismiss && (
          <button
            onClick={onDismiss}
            className="ml-auto px-2 py-0.5 text-xs text-cyan-600 hover:text-cyan-700 border border-cyan-500/30 rounded hover:bg-cyan-500/10 transition-colors"
          >
            Dismiss
          </button>
        )}
      </div>
    </div>
  );
}