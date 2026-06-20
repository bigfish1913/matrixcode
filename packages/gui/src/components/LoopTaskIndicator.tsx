import React from 'react';

interface LoopTask {
  message: string;
  intervalSeconds: number;
  count: number;
  maxCount?: number;
  isActive: boolean;
}

interface CronTask {
  id: number;
  message: string;
  minuteInterval: number;
  isActive: boolean;
}

interface LoopTaskIndicatorProps {
  loopTask?: LoopTask | null;  // Allow both undefined and null
  cronTasks: CronTask[];
  onStopLoop?: () => void;
  onStopCron?: (id: number) => void;
}

export function LoopTaskIndicator({
  loopTask,
  cronTasks,
  onStopLoop,
  onStopCron,
}: LoopTaskIndicatorProps) {
  const hasActiveTasks = loopTask?.isActive || cronTasks.some(t => t.isActive);

  if (!hasActiveTasks) return null;

  const formatInterval = (seconds: number): string => {
    if (seconds < 60) return `${seconds}s`;
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m`;
    return `${Math.floor(seconds / 3600)}h`;
  };

  return (
    <div className="fixed bottom-20 right-4 z-40 space-y-2">
      {/* Loop Task */}
      {loopTask?.isActive && (
        <div className="bg-card border shadow-lg rounded-lg p-3 max-w-sm">
          <div className="flex items-center justify-between mb-2">
            <div className="flex items-center gap-2">
              <span className="text-primary animate-pulse">●</span>
              <span className="font-medium text-sm">循环任务</span>
            </div>
            {onStopLoop && (
              <button
                onClick={onStopLoop}
                className="text-xs px-2 py-1 hover:bg-destructive hover:text-destructive-foreground rounded transition-colors"
              >
                停止
              </button>
            )}
          </div>
          <div className="text-xs text-muted-foreground space-y-1">
            <div className="truncate">
              <span className="font-medium">内容: </span>
              {loopTask.message}
            </div>
            <div className="flex gap-2">
              <span>
                <span className="font-medium">间隔: </span>
                {formatInterval(loopTask.intervalSeconds)}
              </span>
              <span>
                <span className="font-medium">次数: </span>
                {loopTask.count}
                {loopTask.maxCount && ` / ${loopTask.maxCount}`}
              </span>
            </div>
          </div>
          {/* Progress bar */}
          {loopTask.maxCount && (
            <div className="mt-2 h-1 bg-accent rounded-full overflow-hidden">
              <div
                className="h-full bg-primary transition-all"
                style={{ width: `${(loopTask.count / loopTask.maxCount) * 100}%` }}
              />
            </div>
          )}
        </div>
      )}

      {/* Cron Tasks */}
      {cronTasks.filter(t => t.isActive).length > 0 && (
        <div className="bg-card border shadow-lg rounded-lg p-3 max-w-sm">
          <div className="flex items-center justify-between mb-2">
            <div className="flex items-center gap-2">
              <span className="text-primary animate-pulse">●</span>
              <span className="font-medium text-sm">
                定时任务 ({cronTasks.filter(t => t.isActive).length})
              </span>
            </div>
          </div>
          <div className="space-y-2">
            {cronTasks.filter(t => t.isActive).map(task => (
              <div key={task.id} className="border rounded p-2 bg-background">
                <div className="flex items-center justify-between">
                  <div className="text-xs truncate flex-1">
                    <span className="text-muted-foreground">#{task.id}: </span>
                    {task.message}
                  </div>
                  {onStopCron && (
                    <button
                      onClick={() => onStopCron(task.id)}
                      className="text-xs px-1.5 py-0.5 hover:bg-destructive hover:text-destructive-foreground rounded transition-colors ml-2"
                    >
                      停止
                    </button>
                  )}
                </div>
                <div className="text-xs text-muted-foreground mt-1">
                  每 {task.minuteInterval} 分钟
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}