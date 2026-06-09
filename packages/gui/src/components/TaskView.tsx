import React, { useEffect, useState } from 'react';
import { useTaskStore, type TaskInfo, type TaskStatus } from '../stores/taskStore';

function StatusBadge({ status }: { status: TaskStatus }) {
  const config: Record<TaskStatus, { label: string; className: string }> = {
    pending: { label: 'Pending', className: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-300' },
    in_progress: { label: 'Running', className: 'bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-300' },
    completed: { label: 'Done', className: 'bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-300' },
    failed: { label: 'Failed', className: 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-300' },
    paused: { label: 'Paused', className: 'bg-gray-100 text-gray-800 dark:bg-gray-900/30 dark:text-gray-300' },
  };

  const { label, className } = config[status] || config.pending;

  return (
    <span className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${className}`}>
      {status === 'in_progress' && (
        <span className="w-1.5 h-1.5 rounded-full bg-current mr-1.5 animate-pulse" />
      )}
      {label}
    </span>
  );
}

function TaskCard({ task }: { task: TaskInfo }) {
  const cancelTask = useTaskStore((s) => s.cancelTask);
  const pauseTask = useTaskStore((s) => s.pauseTask);
  const resumeTask = useTaskStore((s) => s.resumeTask);
  const [expanded, setExpanded] = useState(false);

  const progressPercent = task.progress != null ? Math.round(task.progress * 100) : null;

  return (
    <div className="border rounded-lg p-3 hover:shadow-sm transition-shadow">
      <div className="flex items-start justify-between gap-2">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <h3 className="text-sm font-medium truncate">{task.title}</h3>
            <StatusBadge status={task.status} />
          </div>
          {task.description && (
            <p className="text-xs text-muted-foreground mt-1 line-clamp-2">{task.description}</p>
          )}
        </div>
        <div className="flex items-center gap-1 shrink-0">
          {task.status === 'in_progress' && (
            <>
              <button
                onClick={() => pauseTask(task.id)}
                className="px-2 py-1 text-xs border rounded hover:bg-accent transition-colors"
                title="Pause"
              >
                ⏸
              </button>
              <button
                onClick={() => cancelTask(task.id)}
                className="px-2 py-1 text-xs border rounded hover:bg-destructive/10 text-destructive transition-colors"
                title="Cancel"
              >
                ✕
              </button>
            </>
          )}
          {task.status === 'paused' && (
            <button
              onClick={() => resumeTask(task.id)}
              className="px-2 py-1 text-xs border rounded hover:bg-accent transition-colors"
              title="Resume"
            >
              ▶
            </button>
          )}
          <button
            onClick={() => setExpanded(!expanded)}
            className="px-2 py-1 text-xs border rounded hover:bg-accent transition-colors"
          >
            {expanded ? '▲' : '▼'}
          </button>
        </div>
      </div>

      {/* Progress bar */}
      {progressPercent != null && (
        <div className="mt-2">
          <div className="flex items-center justify-between text-xs text-muted-foreground mb-1">
            <span>Progress</span>
            <span>{progressPercent}%</span>
          </div>
          <div className="w-full h-1.5 bg-muted rounded-full overflow-hidden">
            <div
              className={`h-full rounded-full transition-all ${
                task.status === 'completed'
                  ? 'bg-green-500'
                  : task.status === 'failed'
                    ? 'bg-red-500'
                    : 'bg-primary'
              }`}
              style={{ width: `${progressPercent}%` }}
            />
          </div>
        </div>
      )}

      {/* Expanded details */}
      {expanded && (
        <div className="mt-3 pt-3 border-t text-xs space-y-1 text-muted-foreground">
          <div><span className="font-medium">ID:</span> {task.id}</div>
          {task.session_id && <div><span className="font-medium">Session:</span> {task.session_id}</div>}
          {task.created_at && <div><span className="font-medium">Created:</span> {task.created_at}</div>}
          {task.updated_at && <div><span className="font-medium">Updated:</span> {task.updated_at}</div>}
          {task.error && (
            <div className="text-red-600 dark:text-red-400">
              <span className="font-medium">Error:</span> {task.error}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export function TaskView() {
  const tasks = useTaskStore((s) => s.tasks);
  const loadTasks = useTaskStore((s) => s.loadTasks);
  const [filter, setFilter] = useState<TaskStatus | 'all'>('all');

  useEffect(() => {
    loadTasks();
  }, [loadTasks]);

  const filteredTasks = filter === 'all'
    ? tasks
    : tasks.filter((t) => t.status === filter);

  const activeTasks = tasks.filter((t) => t.status === 'in_progress' || t.status === 'paused');
  const completedTasks = tasks.filter((t) => t.status === 'completed');
  const failedTasks = tasks.filter((t) => t.status === 'failed');

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="border-b p-4">
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-bold">Tasks</h2>
          <div className="flex items-center gap-3 text-xs text-muted-foreground">
            {activeTasks.length > 0 && <span>{activeTasks.length} active</span>}
            {completedTasks.length > 0 && <span>{completedTasks.length} done</span>}
            {failedTasks.length > 0 && <span className="text-red-500">{failedTasks.length} failed</span>}
          </div>
        </div>

        {/* Filter tabs */}
        <div className="flex gap-1 mt-3">
          {[
            { key: 'all' as const, label: 'All' },
            { key: 'in_progress' as const, label: 'Running' },
            { key: 'paused' as const, label: 'Paused' },
            { key: 'completed' as const, label: 'Done' },
            { key: 'failed' as const, label: 'Failed' },
          ].map(({ key, label }) => (
            <button
              key={key}
              onClick={() => setFilter(key)}
              className={`px-2.5 py-1 text-xs rounded-md transition-colors ${
                filter === key
                  ? 'bg-primary text-primary-foreground'
                  : 'hover:bg-accent text-muted-foreground'
              }`}
            >
              {label}
            </button>
          ))}
        </div>
      </div>

      {/* Task list */}
      <div className="flex-1 overflow-y-auto p-4">
        {filteredTasks.length === 0 && (
          <div className="flex items-center justify-center h-full text-muted-foreground">
            <p className="text-sm">
              {tasks.length === 0
                ? 'No tasks yet. Tasks will appear here when running agent workflows.'
                : 'No tasks match the current filter.'}
            </p>
          </div>
        )}
        <div className="space-y-2">
          {filteredTasks.map((task) => (
            <TaskCard key={task.id} task={task} />
          ))}
        </div>
      </div>
    </div>
  );
}
