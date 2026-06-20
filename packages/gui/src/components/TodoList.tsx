import React from 'react';

// Todo item matching TUI TodoWrite tool
export interface TodoItem {
  id: string;
  content: string;
  status: 'pending' | 'in_progress' | 'completed';
  priority?: 'high' | 'medium' | 'low';
}

interface TodoListProps {
  todos: TodoItem[];
  maxVisible?: number;
}

// Todo status icons matching TUI
const TODO_STATUS_ICONS: Record<string, { icon: string; color: string }> = {
  pending: { icon: '[ ]', color: 'text-gray-400' },
  in_progress: { icon: '[~]', color: 'text-yellow-500 animate-pulse' },
  completed: { icon: '[x]', color: 'text-green-500' },
};

// Priority indicators
const TODO_PRIORITY_COLORS: Record<string, string> = {
  high: 'text-red-500',
  medium: 'text-yellow-500',
  low: 'text-gray-400',
};

// Single todo item
function TodoItemView({ todo }: { todo: TodoItem }) {
  const statusInfo = TODO_STATUS_ICONS[todo.status];

  return (
    <div className="flex items-center gap-2 py-1 px-2 rounded hover:bg-accent/30 transition-colors">
      {/* Status icon */}
      <span className={`font-mono text-sm ${statusInfo.color}`}>
        {statusInfo.icon}
      </span>

      {/* Priority indicator */}
      {todo.priority && (
        <span className={`text-xs ${TODO_PRIORITY_COLORS[todo.priority]}`}>
          ●
        </span>
      )}

      {/* Content */}
      <span className={`text-sm ${todo.status === 'completed' ? 'text-muted-foreground line-through' : 'text-foreground'}`}>
        {todo.content}
      </span>

      {/* Status label */}
      {todo.status === 'in_progress' && (
        <span className="text-xs text-yellow-500 ml-auto">
          进行中
        </span>
      )}
    </div>
  );
}

// Todo progress bar
function TodoProgressBar({ todos }: { todos: TodoItem[] }) {
  const completed = todos.filter(t => t.status === 'completed').length;
  const total = todos.length;
  const percentage = total > 0 ? (completed / total) * 100 : 0;

  return (
    <div className="flex items-center gap-2 mb-2">
      <div className="flex-1 h-2 bg-muted rounded-full overflow-hidden">
        <div
          className="h-full bg-green-500 transition-all duration-300"
          style={{ width: `${percentage}%` }}
        />
      </div>
      <span className="text-xs text-muted-foreground font-mono">
        {completed}/{total}
      </span>
      <span className="text-xs text-muted-foreground">
        ({Math.round(percentage)}%)
      </span>
    </div>
  );
}

export function TodoList({ todos, maxVisible = 10 }: TodoListProps) {
  if (todos.length === 0) return null;

  // Sort: in_progress first, then pending, then completed
  const sortedTodos = [...todos].sort((a, b) => {
    const order = { in_progress: 0, pending: 1, completed: 2 };
    return order[a.status] - order[b.status];
  });

  // Limit visible items
  const visibleTodos = sortedTodos.slice(0, maxVisible);
  const hasMore = sortedTodos.length > maxVisible;

  // Count by status
  const inProgressCount = todos.filter(t => t.status === 'in_progress').length;
  const pendingCount = todos.filter(t => t.status === 'pending').length;
  const completedCount = todos.filter(t => t.status === 'completed').length;

  return (
    <div className="bg-card border rounded-lg p-3 mb-3">
      {/* Header */}
      <div className="flex items-center gap-2 mb-2">
        <span className="text-lg">📋</span>
        <span className="font-semibold text-sm">任务列表</span>
        <div className="flex-1 flex justify-end gap-2 text-xs text-muted-foreground">
          {inProgressCount > 0 && (
            <span className="text-yellow-500">{inProgressCount} 进行中</span>
          )}
          {pendingCount > 0 && (
            <span className="text-gray-400">{pendingCount} 待处理</span>
          )}
          {completedCount > 0 && (
            <span className="text-green-500">{completedCount} 完成</span>
          )}
        </div>
      </div>

      {/* Progress bar */}
      <TodoProgressBar todos={todos} />

      {/* Todo items */}
      <div className="space-y-0.5">
        {visibleTodos.map((todo) => (
          <TodoItemView key={todo.id} todo={todo} />
        ))}
      </div>

      {/* More items indicator */}
      {hasMore && (
        <div className="text-xs text-muted-foreground text-center py-1 mt-1 border-t">
          还有 {sortedTodos.length - maxVisible} 项...
        </div>
      )}
    </div>
  );
}

// Compact todo indicator for hint bar
export function TodoIndicator({ todos: todoList }: { todos: TodoItem[] }) {
  if (todoList.length === 0) return null;

  const completed = todoList.filter(t => t.status === 'completed').length;
  const inProgress = todoList.filter(t => t.status === 'in_progress').length;
  const total = todoList.length;

  return (
    <span className="flex items-center gap-1.5 text-xs">
      <span>📋</span>
      <span className="text-muted-foreground">
        {completed}/{total}
      </span>
      {inProgress > 0 && (
        <span className="text-yellow-500 animate-pulse">
          ({inProgress} 进行中)
        </span>
      )}
    </span>
  );
}