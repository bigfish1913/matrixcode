import React, { useEffect, useState, useRef } from 'react';
import { useSessionStore, type SessionInfo } from '../stores/sessionStore';
import { useChatStore } from '../stores/chatStore';
import { getVersion } from '@tauri-apps/api/app';

export type ViewType = 'chat' | 'tasks' | 'settings';

interface SidebarProps {
  currentView: ViewType;
  onViewChange: (view: ViewType) => void;
}

// Inline editable session name with delete button and selection checkbox
function SessionNameEdit({
  session,
  isSelected,
  isSelectionMode,
  isChecked,
  onSelect,
  onDelete,
  onToggleCheck,
}: {
  session: SessionInfo;
  isSelected: boolean;
  isSelectionMode: boolean;
  isChecked: boolean;
  onSelect: () => void;
  onDelete: () => void;
  onToggleCheck: () => void;
}) {
  const renameSession = useSessionStore((s) => s.renameSession);
  const [isEditing, setIsEditing] = useState(false);
  const [name, setName] = useState(session.name);
  const [showDelete, setShowDelete] = useState(false);

  const handleSave = async () => {
    if (name.trim() && name !== session.name) {
      await renameSession(name.trim());
    }
    setIsEditing(false);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      handleSave();
    } else if (e.key === 'Escape') {
      setName(session.name);
      setIsEditing(false);
    }
  };

  const handleClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (isSelectionMode) {
      onToggleCheck();
    } else {
      onSelect();
    }
  };

  const handleDoubleClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!isSelectionMode && isSelected) {
      setIsEditing(true);
    }
  };

  const handleDeleteClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (confirm(`确定删除会话 "${session.name || session.id.slice(0, 8)}"？`)) {
      onDelete();
    }
  };

  if (isEditing) {
    return (
      <input
        type="text"
        value={name}
        onChange={(e) => setName(e.target.value)}
        onBlur={handleSave}
        onKeyDown={handleKeyDown}
        autoFocus
        onClick={(e) => e.stopPropagation()}
        className="w-full bg-background px-1 py-0.5 text-sm rounded border focus:outline-none focus:ring-1 focus:ring-primary"
      />
    );
  }

  return (
    <div
      className="truncate cursor-pointer flex items-center gap-2 group"
      onClick={handleClick}
      onDoubleClick={handleDoubleClick}
      onMouseEnter={() => setShowDelete(true)}
      onMouseLeave={() => setShowDelete(false)}
      title={isSelectionMode ? '点击选择' : (isSelected ? '双击编辑标题' : session.name)}
    >
      {/* Checkbox in selection mode */}
      {isSelectionMode && (
        <input
          type="checkbox"
          checked={isChecked}
          onChange={onToggleCheck}
          onClick={(e) => e.stopPropagation()}
          className="w-4 h-4 rounded border-gray-300 text-primary focus:ring-primary"
        />
      )}
      <span className="flex-1 truncate">
        {session.name || `Session ${session.id.slice(0, 8)}`}
      </span>
      {/* Delete button - visible on hover (only in non-selection mode) */}
      {!isSelectionMode && showDelete && (
        <button
          onClick={handleDeleteClick}
          className="text-xs px-1 py-0.5 text-red-500 hover:bg-red-500/10 rounded opacity-0 group-hover:opacity-100 transition-opacity"
          title="删除会话"
          aria-label={`删除会话 ${session.name}`}
        >
          ✕
        </button>
      )}
    </div>
  );
}

export function Sidebar({ currentView, onViewChange }: SidebarProps) {
  const sessions = useSessionStore((s) => s.sessions);
  const currentSessionId = useSessionStore((s) => s.currentSessionId);
  const loadSessions = useSessionStore((s) => s.loadSessions);
  const loading = useSessionStore((s) => s.loading);
  const error = useSessionStore((s) => s.error);
  const clearError = useSessionStore((s) => s.clearError);
  const createSession = useSessionStore((s) => s.createSession);
  const continueLast = useSessionStore((s) => s.continueLast);
  const switchSession = useSessionStore((s) => s.switchSession);
  const deleteSession = useSessionStore((s) => s.deleteSession);
  const batchDeleteSessions = useSessionStore((s) => s.batchDeleteSessions);
  const selectionMode = useSessionStore((s) => s.selectionMode);
  const selectedIds = useSessionStore((s) => s.selectedIds);
  const toggleSelectionMode = useSessionStore((s) => s.toggleSelectionMode);
  const toggleSelection = useSessionStore((s) => s.toggleSelection);
  const selectAll = useSessionStore((s) => s.selectAll);
  const clearSelection = useSessionStore((s) => s.clearSelection);
  const clearMessages = useChatStore((s) => s.clearMessages);
  const loadMessages = useChatStore((s) => s.loadMessages);
  const [collapsed, setCollapsed] = useState(false);
  const [isOperating, setIsOperating] = useState(false); // Track async operations
  const [appVersion, setAppVersion] = useState('0.1.0');
  const sessionListRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    loadSessions();
    // Get app version
    getVersion().then(setAppVersion).catch(() => setAppVersion('0.1.0'));
  }, [loadSessions]);

  // Scroll to current session when it changes
  useEffect(() => {
    if (currentSessionId && sessionListRef.current) {
      const selectedBtn = sessionListRef.current.querySelector(`[data-session-id="${currentSessionId}"]`);
      if (selectedBtn) {
        selectedBtn.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
      }
    }
  }, [currentSessionId, sessions]);

  const handleNew = async () => {
    setIsOperating(true);
    try {
      await createSession();
      clearMessages();
      onViewChange('chat');
    } finally {
      setIsOperating(false);
    }
  };

  const handleContinue = async () => {
    setIsOperating(true);
    try {
      const id = await continueLast();
      if (id) {
        // Load messages from the resumed session
        await loadMessages();
      }
      onViewChange('chat');
    } finally {
      setIsOperating(false);
    }
  };

  const handleSelect = async (session: SessionInfo) => {
    setIsOperating(true);
    try {
      // Actually switch the backend session and load its messages
      await switchSession(session.id);
      await loadMessages();
      onViewChange('chat');
    } finally {
      setIsOperating(false);
    }
  };

  const handleDelete = async (id: string) => {
    setIsOperating(true);
    try {
      await deleteSession(id);
    } finally {
      setIsOperating(false);
    }
  };

  const handleBatchDelete = async () => {
    const ids = Array.from(selectedIds);
    if (ids.length === 0) return;
    
    const confirmMsg = ids.length === 1 
      ? `确定删除选中的会话？`
      : `确定删除 ${ids.length} 个会话？`;
    
    if (!confirm(confirmMsg)) return;
    
    setIsOperating(true);
    try {
      await batchDeleteSessions(ids);
    } finally {
      setIsOperating(false);
    }
  };

  if (collapsed) {
    return (
      <div className="w-12 border-r flex flex-col items-center py-3 gap-3 bg-card">
        <button
          onClick={() => setCollapsed(false)}
          className="p-1.5 hover:bg-accent rounded text-sm"
          title="展开侧边栏"
        >
          ▶
        </button>
        <button
          onClick={handleNew}
          disabled={isOperating}
          className="p-1.5 hover:bg-accent rounded text-sm disabled:opacity-50"
          title="新建对话"
        >
          +
        </button>
        <button
          onClick={() => onViewChange('tasks')}
          className={`p-1.5 rounded text-sm ${currentView === 'tasks' ? 'bg-accent' : 'hover:bg-accent'}`}
          title="任务列表"
        >
          ☐
        </button>
        <button
          onClick={() => onViewChange('settings')}
          className={`p-1.5 rounded text-sm ${currentView === 'settings' ? 'bg-accent' : 'hover:bg-accent'}`}
          title="设置"
        >
          ⚙
        </button>
      </div>
    );
  }

  return (
    <div className="w-64 border-r flex flex-col h-full bg-card">
      {/* Header */}
      <div className="p-3 border-b">
        <div className="flex items-center justify-between">
          <h1 className="text-lg font-bold">MatrixCode</h1>
          <button
            onClick={() => setCollapsed(true)}
            className="p-1 hover:bg-accent rounded text-xs text-muted-foreground"
            title="折叠侧边栏"
          >
            ◀
          </button>
        </div>
        <div className="flex gap-2 mt-2">
          <button
            onClick={handleNew}
            disabled={isOperating}
            className="flex-1 text-xs px-2 py-1.5 bg-primary text-primary-foreground rounded hover:bg-primary/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            title="创建新的对话会话"
          >
            {isOperating ? '处理中...' : '新建对话'}
          </button>
          <button
            onClick={handleContinue}
            disabled={isOperating}
            className="text-xs px-2 py-1.5 border rounded hover:bg-accent transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            title="恢复上次对话"
          >
            {isOperating ? '...' : '继续'}
          </button>
        </div>

        {/* Error message */}
        {error && (
          <div className="mt-2 px-2 py-1 bg-red-500/10 text-red-600 rounded text-xs flex items-center justify-between animate-fade-in">
            <span>{error}</span>
            <button onClick={clearError} className="ml-2 hover:underline">清除</button>
          </div>
        )}
      </div>

      {/* Navigation tabs */}
      <div className="flex border-b">
        {[
          { key: 'chat' as ViewType, label: '💬 对话' },
          { key: 'tasks' as ViewType, label: '☐ 任务' },
          { key: 'settings' as ViewType, label: '⚙ 设置' },
        ].map(({ key, label }) => (
          <button
            key={key}
            onClick={() => onViewChange(key)}
            className={`flex-1 text-xs py-2 transition-colors ${
              currentView === key
                ? 'border-b-2 border-primary font-medium'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            {label}
          </button>
        ))}
      </div>

      {/* Session list (only shown in chat view) */}
      {currentView === 'chat' && (
        <>
          {/* Selection mode toolbar */}
          {selectionMode && (
            <div className="p-2 border-b bg-accent/50 flex items-center gap-2">
              <span className="text-xs text-muted-foreground">
                已选择 {selectedIds.size} / {sessions.length}
              </span>
              <button
                onClick={selectAll}
                disabled={sessions.length === 0}
                className="text-xs px-2 py-1 border rounded hover:bg-accent disabled:opacity-50"
                title="全选"
              >
                全选
              </button>
              <button
                onClick={clearSelection}
                disabled={selectedIds.size === 0}
                className="text-xs px-2 py-1 border rounded hover:bg-accent disabled:opacity-50"
                title="取消选择"
              >
                取消
              </button>
              <button
                onClick={handleBatchDelete}
                disabled={selectedIds.size === 0 || isOperating}
                className="text-xs px-2 py-1 bg-red-500 text-white rounded hover:bg-red-600 disabled:opacity-50 disabled:cursor-not-allowed"
                title="删除选中"
              >
                {isOperating ? '删除中...' : `删除 (${selectedIds.size})`}
              </button>
              <button
                onClick={toggleSelectionMode}
                className="text-xs px-2 py-1 border rounded hover:bg-accent ml-auto"
                title="退出选择模式"
              >
                完成
              </button>
            </div>
          )}
          
          {/* Selection mode toggle button */}
          {!selectionMode && (
            <div className="px-3 py-1 border-b">
              <button
                onClick={toggleSelectionMode}
                disabled={sessions.length === 0}
                className="text-xs text-muted-foreground hover:text-foreground disabled:opacity-50"
                title="批量管理"
              >
                🗑️ 批量管理
              </button>
            </div>
          )}
        
          <div ref={sessionListRef} className="flex-1 overflow-y-auto">
            {loading && (
              <div className="flex items-center justify-center py-4 text-xs text-muted-foreground">
                <span className="animate-pulse">加载中...</span>
              </div>
            )}
            {!loading && error && sessions.length === 0 && (
              <div className="text-xs text-red-500 p-3 flex flex-col gap-2">
                <span>加载失败</span>
                <button onClick={loadSessions} className="text-xs underline hover:no-underline">
                  重试
                </button>
              </div>
            )}
            {!loading && !error && sessions.length === 0 && (
              <p className="text-xs text-muted-foreground p-3">暂无会话</p>
            )}
            {sessions.map((s) => (
              <button
                key={s.id}
                data-session-id={s.id}
                onClick={() => selectionMode ? toggleSelection(s.id) : handleSelect(s)}
                disabled={isOperating && !selectionMode}
                className={`w-full text-left px-3 py-2 text-sm border-b hover:bg-accent transition-colors disabled:opacity-50 ${
                  selectionMode && selectedIds.has(s.id)
                    ? 'bg-primary/10 border-l-2 border-l-primary'
                    : ''
                } ${
                  !selectionMode && s.id === currentSessionId
                    ? 'bg-accent font-medium'
                    : ''
                }`}
              >
                <SessionNameEdit
                  session={s}
                  isSelected={s.id === currentSessionId}
                  isSelectionMode={selectionMode}
                  isChecked={selectedIds.has(s.id)}
                  onSelect={() => handleSelect(s)}
                  onDelete={() => handleDelete(s.id)}
                  onToggleCheck={() => toggleSelection(s.id)}
                />
                <div className="text-xs text-muted-foreground">
                  {s.message_count} 条消息 · {s.created_at}
                </div>
              </button>
            ))}
          </div>
        </>
      )}

      {currentView !== 'chat' && (
        <div className="flex-1" />
      )}

      {/* Footer */}
      <div className="p-3 border-t text-xs text-muted-foreground">
        v{appVersion}
      </div>
    </div>
  );
}