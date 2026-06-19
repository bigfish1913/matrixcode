import React from 'react';

interface StatsPanelProps {
  apiCalls: number;
  toolCalls: number;
  compressions: number;
  memorySaves: number;
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheCreationTokens: number;
  status: 'idle' | 'running' | 'error';
  activityType?: string;
  elapsedSeconds?: number;
}

export function StatsPanel({
  apiCalls,
  toolCalls,
  compressions,
  memorySaves,
  inputTokens,
  outputTokens,
  cacheReadTokens,
  cacheCreationTokens,
  status,
  activityType,
  elapsedSeconds,
}: StatsPanelProps) {
  const formatNumber = (num: number): string => {
    if (num >= 1000000) return `${(num / 1000000).toFixed(1)}M`;
    if (num >= 1000) return `${(num / 1000).toFixed(1)}k`;
    return String(num);
  };

  const formatTime = (seconds: number): string => {
    if (seconds < 60) return `${seconds.toFixed(1)}s`;
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${mins}m ${secs.toFixed(0)}s`;
  };

  const totalTokens = inputTokens + outputTokens;
  const efficiency = cacheReadTokens > 0
    ? ((cacheReadTokens / totalTokens) * 100).toFixed(1)
    : '0';

  return (
    <div className="grid grid-cols-2 md:grid-cols-4 gap-3 p-4 bg-muted/20">
      {/* API Calls */}
      <div className="flex flex-col gap-1">
        <div className="text-xs text-muted-foreground">API Calls</div>
        <div className="text-lg font-bold text-blue-600">
          {formatNumber(apiCalls)}
        </div>
      </div>

      {/* Tool Calls */}
      <div className="flex flex-col gap-1">
        <div className="text-xs text-muted-foreground">Tool Calls</div>
        <div className="text-lg font-bold text-amber-600">
          {formatNumber(toolCalls)}
        </div>
      </div>

      {/* Compressions */}
      <div className="flex flex-col gap-1">
        <div className="text-xs text-muted-foreground">Compressions</div>
        <div className="text-lg font-bold text-cyan-600">
          {formatNumber(compressions)}
        </div>
      </div>

      {/* Memory Saves */}
      <div className="flex flex-col gap-1">
        <div className="text-xs text-muted-foreground">Memory Saves</div>
        <div className="text-lg font-bold text-purple-600">
          {formatNumber(memorySaves)}
        </div>
      </div>

      {/* Divider */}
      <div className="col-span-full border-t my-2" />

      {/* Input Tokens */}
      <div className="flex flex-col gap-1">
        <div className="text-xs text-muted-foreground">Input Tokens</div>
        <div className="text-sm font-medium text-green-600">
          {formatNumber(inputTokens)}
        </div>
      </div>

      {/* Output Tokens */}
      <div className="flex flex-col gap-1">
        <div className="text-xs text-muted-foreground">Output Tokens</div>
        <div className="text-sm font-medium text-green-600">
          {formatNumber(outputTokens)}
        </div>
      </div>

      {/* Cache Read */}
      <div className="flex flex-col gap-1">
        <div className="text-xs text-muted-foreground">Cache Read</div>
        <div className="text-sm font-medium text-teal-600">
          {formatNumber(cacheReadTokens)}
        </div>
      </div>

      {/* Cache Created */}
      <div className="flex flex-col gap-1">
        <div className="text-xs text-muted-foreground">Cache Created</div>
        <div className="text-sm font-medium text-teal-600">
          {formatNumber(cacheCreationTokens)}
        </div>
      </div>

      {/* Efficiency */}
      {cacheReadTokens > 0 && (
        <div className="col-span-2 flex flex-col gap-1">
          <div className="text-xs text-muted-foreground">Cache Efficiency</div>
          <div className="text-sm font-medium text-emerald-600">
            {efficiency}% saved
          </div>
        </div>
      )}

      {/* Current Activity */}
      {status === 'running' && activityType && elapsedSeconds && (
        <div className="col-span-2 flex flex-col gap-1">
          <div className="text-xs text-muted-foreground">Current Activity</div>
          <div className="flex items-center gap-2">
            <span className="animate-spin">🔄</span>
            <span className="text-sm font-medium capitalize">{activityType}</span>
            <span className="text-xs text-muted-foreground">
              ({formatTime(elapsedSeconds)})
            </span>
          </div>
        </div>
      )}
    </div>
  );
}