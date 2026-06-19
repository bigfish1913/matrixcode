import React from 'react';

interface DebugLog {
  category: string;
  message: string;
  timestamp: number;
}

interface DebugPanelProps {
  logs: DebugLog[];
  apiCalls?: number;
  compressions?: number;
  toolCalls?: number;
  memorySaves?: number;
}

export function DebugPanel({ logs, apiCalls, compressions, toolCalls, memorySaves }: DebugPanelProps) {
  if (logs.length === 0) {
    return null;
  }

  const formatTime = (timestamp: number) => {
    const date = new Date(timestamp);
    return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  };

  const getCategoryColor = (category: string): string => {
    switch (category.toLowerCase()) {
      case 'api':
        return 'text-blue-500';
      case 'tool':
        return 'text-amber-500';
      case 'memory':
        return 'text-purple-500';
      case 'compression':
        return 'text-cyan-500';
      case 'error':
        return 'text-red-500';
      default:
        return 'text-muted-foreground';
    }
  };

  return (
    <div className="border-t bg-muted/20">
      {/* Stats header */}
      <div className="px-4 py-1 border-b bg-muted/50 flex gap-4 text-xs">
        <span className="text-muted-foreground">Debug Stats:</span>
        {apiCalls !== undefined && (
          <span><span className="text-blue-500 font-medium">{apiCalls}</span> API calls</span>
        )}
        {compressions !== undefined && (
          <span><span className="text-cyan-500 font-medium">{compressions}</span> Compressions</span>
        )}
        {toolCalls !== undefined && (
          <span><span className="text-amber-500 font-medium">{toolCalls}</span> Tool calls</span>
        )}
        {memorySaves !== undefined && (
          <span><span className="text-purple-500 font-medium">{memorySaves}</span> Memory saves</span>
        )}
      </div>

      {/* Logs */}
      <div className="max-h-[200px] overflow-y-auto px-4 py-2 space-y-1">
        {logs.slice(-20).reverse().map((log, idx) => (
          <div key={idx} className="flex gap-2 text-xs">
            <span className="font-mono text-muted-foreground opacity-70">
              {formatTime(log.timestamp)}
            </span>
            <span className={`font-medium ${getCategoryColor(log.category)}`}>
              [{log.category}]
            </span>
            <span className="text-foreground flex-1 truncate">
              {log.message}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}