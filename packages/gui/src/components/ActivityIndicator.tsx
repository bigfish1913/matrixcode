import React from 'react';

// Activity types matching TUI Activity enum
export type ActivityType =
  | 'idle'
  | 'thinking'
  | 'reading'
  | 'writing'
  | 'editing'
  | 'searching'
  | 'running'
  | 'websearch'
  | 'webfetch'
  | 'tool'
  | 'asking';

interface ActivityIndicatorProps {
  activity: ActivityType;
  detail?: string;
  elapsedSeconds?: number;
}

// Activity labels and colors (matching TUI Activity::label() and Activity::color())
const ACTIVITY_CONFIG: Record<ActivityType, { label: string; color: string; icon: string }> = {
  idle: { label: '就绪', color: 'text-green-500', icon: '●' },
  thinking: { label: '思考中', color: 'text-purple-500', icon: '💭' },
  reading: { label: '读取', color: 'text-cyan-500', icon: '📖' },
  writing: { label: '写入', color: 'text-yellow-500', icon: '✍️' },
  editing: { label: '编辑', color: 'text-yellow-500', icon: '📝' },
  searching: { label: '搜索', color: 'text-cyan-500', icon: '🔍' },
  running: { label: '执行', color: 'text-red-500', icon: '⚡' },
  websearch: { label: '网络搜索', color: 'text-blue-500', icon: '🌐' },
  webfetch: { label: '网络获取', color: 'text-blue-500', icon: '⬇️' },
  tool: { label: '工具', color: 'text-cyan-500', icon: '🔧' },
  asking: { label: '等待响应', color: 'text-red-500', icon: '❓' },
};

// Format elapsed time
function formatElapsed(seconds: number): string {
  if (seconds < 60) {
    return `${Math.floor(seconds)}s`;
  }
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds % 60);
  return `${mins}:${secs.toString().padStart(2, '0')}`;
}

export function ActivityIndicator({ activity, detail, elapsedSeconds }: ActivityIndicatorProps) {
  const config = ACTIVITY_CONFIG[activity] || ACTIVITY_CONFIG.idle;
  const showElapsed = elapsedSeconds && elapsedSeconds > 0;

  return (
    <div className="flex items-center gap-2 px-4 py-2 bg-card border rounded-lg mb-2">
      {/* Activity icon */}
      <span className={`${config.color} text-lg animate-pulse`}>
        {config.icon}
      </span>

      {/* Activity label */}
      <span className={`font-medium ${config.color}`}>
        {config.label}
      </span>

      {/* Detail (tool name, file name, etc.) */}
      {detail && (
        <span className="text-sm text-muted-foreground">
          {detail}
        </span>
      )}

      {/* Elapsed time */}
      {showElapsed && (
        <span className="text-xs text-muted-foreground ml-auto font-mono">
          {formatElapsed(elapsedSeconds)}
        </span>
      )}

      {/* Progress animation */}
      <div className="flex gap-1 ml-2">
        <span className={`w-1.5 h-1.5 rounded-full ${config.color} animate-bounce [animation-delay:-0.3s]`} />
        <span className={`w-1.5 h-1.5 rounded-full ${config.color} animate-bounce [animation-delay:-0.15s]`} />
        <span className={`w-1.5 h-1.5 rounded-full ${config.color} animate-bounce`} />
      </div>
    </div>
  );
}

// Mini activity indicator for status bar
export function MiniActivityIndicator({ activity }: { activity: ActivityType }) {
  const config = ACTIVITY_CONFIG[activity] || ACTIVITY_CONFIG.idle;

  if (activity === 'idle') return null;

  return (
    <div className="flex items-center gap-1">
      <span className={`${config.color} animate-pulse`}>
        {config.icon}
      </span>
      <span className={`text-xs ${config.color}`}>
        {config.label}
      </span>
    </div>
  );
}

// Activity badge for compact display
export function ActivityBadge({ activity, size = 'sm' }: { activity: ActivityType; size?: 'sm' | 'md' }) {
  const config = ACTIVITY_CONFIG[activity] || ACTIVITY_CONFIG.idle;
  const sizeClasses = size === 'sm' ? 'px-2 py-0.5 text-xs' : 'px-3 py-1 text-sm';

  if (activity === 'idle') return null;

  return (
    <div className={`${sizeClasses} rounded-full bg-${config.color}/10 ${config.color} border border-${config.color}/20 flex items-center gap-1`}>
      <span>{config.icon}</span>
      <span>{config.label}</span>
    </div>
  );
}