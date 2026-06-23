import React, { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface SessionInfo {
  id: string;
  name: string;
  message_count: number;
  created_at: string;
  project_path?: string;
  updated_at?: string;
  short_id?: string;
}

interface SessionSwitcherDialogProps {
  onClose: () => void;
  onSelectSession: (sessionId: string) => void;
  currentSessionId?: string | null;
}

export function SessionSwitcherDialog({ onClose, onSelectSession, currentSessionId }: SessionSwitcherDialogProps) {
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [filter, setFilter] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);
  const [isBatchMode, setIsBatchMode] = useState(false);
  const [selectedSessions, setSelectedSessions] = useState<Set<string>>(new Set());
  const [isBatchDeleting, setIsBatchDeleting] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  // Focus input on mount
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const loadSessions = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const sessionList = await invoke<SessionInfo[]>('list_sessions');
      setSessions(sessionList || []);
      // Find the current session index
      if (currentSessionId) {
        const idx = sessionList.findIndex(s => s.id === currentSessionId);
        if (idx >= 0) {
          setSelectedIndex(idx);
        } else {
          setSelectedIndex(0);
        }
      }
    } catch (e) {
      console.error('Failed to load sessions:', e);
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [currentSessionId]);

  // Load sessions from backend
  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  const handleDeleteSession = async (sessionId: string, e: React.MouseEvent) => {
    e.stopPropagation();

    // Confirm deletion
    if (!window.confirm('确定要删除此会话吗？此操作不可恢复。')) {
      return;
    }

    try {
      setDeleting(sessionId);
      await invoke('delete_session', { id: sessionId });
      await loadSessions();
    } catch (err) {
      console.error('Failed to delete session:', err);
      setError(`删除失败: ${err}`);
    } finally {
      setDeleting(null);
    }
  };

  // Toggle session selection for batch operations
  const toggleSessionSelection = (sessionId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    const newSelection = new Set(selectedSessions);
    if (newSelection.has(sessionId)) {
      newSelection.delete(sessionId);
    } else {
      newSelection.add(sessionId);
    }
    setSelectedSessions(newSelection);
  };

  // Select all filtered sessions (excluding current)
  const selectAllFiltered = () => {
    const selectableIds = filteredSessions
      .filter(s => s.id !== currentSessionId)
      .map(s => s.id);
    setSelectedSessions(new Set(selectableIds));
  };

  // Clear selection
  const clearSelection = () => {
    setSelectedSessions(new Set());
  };

  // Batch delete selected sessions
  const handleBatchDelete = async () => {
    if (selectedSessions.size === 0) return;

    const confirmed = window.confirm(
      `确定要删除选中的 ${selectedSessions.size} 个会话吗？此操作不可恢复。`
    );
    if (!confirmed) return;

    try {
      setIsBatchDeleting(true);
      const ids = Array.from(selectedSessions);
      const deletedCount = await invoke<number>('batch_delete_sessions', { ids });
      setSelectedSessions(new Set());
      await loadSessions();
      // Show success message
      setError(null);
      console.log(`Deleted ${deletedCount} sessions`);
    } catch (err) {
      console.error('Batch delete failed:', err);
      setError(`批量删除失败: ${err}`);
    } finally {
      setIsBatchDeleting(false);
    }
  };

  // Filtered sessions
  const filteredSessions = sessions.filter(s =>
    s.name.toLowerCase().includes(filter.toLowerCase()) ||
    s.id.toLowerCase().includes(filter.toLowerCase()) ||
    (s.project_path && s.project_path.toLowerCase().includes(filter.toLowerCase())) ||
    (s.short_id && s.short_id.toLowerCase().includes(filter.toLowerCase()))
  );

  // Keyboard navigation (matching TUI session selection)
  useEffect(() => {
    const handleKeyDown = (e: globalThis.KeyboardEvent) => {
      // In batch mode, Space toggles selection, Enter also toggles (not select session)
      if (isBatchMode) {
        if (e.key === 'ArrowDown' || (e.key === 'j' && !e.ctrlKey && !e.metaKey)) {
          e.preventDefault();
          setSelectedIndex(i => Math.min(i + 1, filteredSessions.length - 1));
        } else if (e.key === 'ArrowUp' || (e.key === 'k' && !e.ctrlKey && !e.metaKey)) {
          e.preventDefault();
          setSelectedIndex(i => Math.max(i - 1, 0));
        } else if (e.key === ' ' || e.key === 'Enter') {
          e.preventDefault();
          if (filteredSessions.length > 0) {
            const session = filteredSessions[selectedIndex];
            if (session.id !== currentSessionId) {
              toggleSessionSelection(session.id, {} as React.MouseEvent);
            }
          }
        } else if (e.key === 'a' && !e.ctrlKey && !e.metaKey) {
          // 'a' to select all
          e.preventDefault();
          selectAllFiltered();
        } else if (e.key === 'c' && !e.ctrlKey && !e.metaKey) {
          // 'c' to clear selection
          e.preventDefault();
          clearSelection();
        } else if (e.key === 'd' && !e.ctrlKey && !e.metaKey) {
          // 'd' to delete selected
          e.preventDefault();
          if (selectedSessions.size > 0) {
            handleBatchDelete();
          }
        } else if (e.key === 'Escape') {
          e.preventDefault();
          if (selectedSessions.size > 0) {
            // First clear selection, then close on second Esc
            clearSelection();
          } else {
            onClose();
          }
        }
      } else {
        // Normal mode navigation
        if (e.key === 'ArrowDown' || (e.key === 'j' && !e.ctrlKey && !e.metaKey)) {
          e.preventDefault();
          setSelectedIndex(i => Math.min(i + 1, filteredSessions.length - 1));
        } else if (e.key === 'ArrowUp' || (e.key === 'k' && !e.ctrlKey && !e.metaKey)) {
          e.preventDefault();
          setSelectedIndex(i => Math.max(i - 1, 0));
        } else if (e.key === 'Enter' && !e.shiftKey) {
          e.preventDefault();
          if (filteredSessions.length > 0) {
            onSelectSession(filteredSessions[selectedIndex].id);
            onClose();
          }
        } else if (e.key === 'Escape') {
          e.preventDefault();
          onClose();
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [filteredSessions, selectedIndex, onClose, onSelectSession, isBatchMode, currentSessionId, selectedSessions]);

  // Handle session click
  const handleSessionClick = (sessionId: string, index: number) => {
    setSelectedIndex(index);
    onSelectSession(sessionId);
    onClose();
  };

  // Extract project name from path
  const getProjectName = (path?: string) => {
    if (!path) return null;
    const parts = path.split(/[/\\]/);
    return parts[parts.length - 1] || path;
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4" onClick={(e) => {
      // Close on background click
      if (e.target === e.currentTarget) {
        onClose();
      }
    }}>
      <div className="bg-card border shadow-lg rounded-lg max-w-2xl w-full max-h-[80vh] flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b">
          <h2 className="text-lg font-semibold">切换会话</h2>
          <div className="flex items-center gap-2">
            {/* Batch mode toggle */}
            <button
              onClick={() => {
                setIsBatchMode(!isBatchMode);
                if (isBatchMode) {
                  setSelectedSessions(new Set());
                }
              }}
              className={`px-3 py-1 text-xs rounded transition-colors ${
                isBatchMode
                  ? 'bg-primary text-primary-foreground'
                  : 'bg-muted hover:bg-accent'
              }`}
              title={isBatchMode ? '退出批量模式' : '进入批量模式'}
            >
              {isBatchMode ? '取消批量' : '批量管理'}
            </button>
            <button
              onClick={onClose}
              className="p-1 hover:bg-accent rounded text-muted-foreground hover:text-foreground transition-colors"
              aria-label="关闭"
            >
              ✕
            </button>
          </div>
        </div>

        {/* Search filter */}
        <div className="p-4 border-b">
          <input
            ref={inputRef}
            type="text"
            value={filter}
            onChange={(e) => {
              setFilter(e.target.value);
              setSelectedIndex(0);  // Reset selection when filter changes
            }}
            placeholder="搜索会话名称、ID 或项目路径..."
            className="w-full px-3 py-2 bg-background border border-input rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-primary/50"
            aria-label="搜索会话"
          />
        </div>

        {/* Batch operations toolbar */}
        {isBatchMode && (
          <div className="px-4 py-2 border-b flex gap-2 flex-wrap bg-muted/30">
            <button
              onClick={selectAllFiltered}
              className="px-3 py-1.5 bg-primary/10 text-primary rounded text-xs hover:bg-primary/20 transition-colors"
            >
              选择全部 ({filteredSessions.filter(s => s.id !== currentSessionId).length})
            </button>
            <button
              onClick={clearSelection}
              className="px-3 py-1.5 bg-muted rounded text-xs hover:bg-accent transition-colors"
            >
              清除选择
            </button>
            <button
              onClick={handleBatchDelete}
              disabled={selectedSessions.size === 0 || isBatchDeleting}
              className="px-3 py-1.5 bg-red-500/10 text-red-500 rounded text-xs hover:bg-red-500/20 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              {isBatchDeleting ? '删除中...' : `删除选中 (${selectedSessions.size})`}
            </button>
          </div>
        )}

        {/* Selection count indicator */}
        {isBatchMode && selectedSessions.size > 0 && (
          <div className="px-4 py-2 bg-primary/10 border-b text-xs text-primary">
            已选择 {selectedSessions.size} 个会话
          </div>
        )}

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-4">
          {loading ? (
            <div className="text-center py-8 text-muted-foreground">
              正在加载会话...
            </div>
          ) : error ? (
            <div className="text-center py-8 text-red-500">
              <p className="mb-2">加载会话失败</p>
              <p className="text-sm">{error}</p>
              <button
                onClick={loadSessions}
                className="mt-4 px-3 py-1.5 bg-primary text-primary-foreground rounded-lg hover:bg-primary/90"
              >
                重试
              </button>
            </div>
          ) : filteredSessions.length === 0 ? (
            <div className="text-center py-8 text-muted-foreground">
              {filter ? '没有匹配的会话' : '没有保存的会话'}
            </div>
          ) : (
            <div className="space-y-2">
              {filteredSessions.map((session, idx) => {
                const isCurrent = session.id === currentSessionId;
                const isDeleting = deleting === session.id;
                const isSelected = selectedSessions.has(session.id);

                return (
                  <div
                    key={session.id}
                    onClick={() => {
                      if (isBatchMode && !isCurrent) {
                        toggleSessionSelection(session.id, {} as React.MouseEvent);
                      } else if (!isDeleting && !isBatchMode) {
                        handleSessionClick(session.id, idx);
                      }
                    }}
                    className={`border rounded-lg p-3 cursor-pointer transition-colors ${
                      isDeleting
                        ? 'opacity-50 cursor-not-allowed'
                        : isSelected
                          ? 'bg-primary/20 border-primary'
                          : idx === selectedIndex && !isBatchMode
                            ? 'bg-primary/10 border-primary'
                            : 'bg-background hover:bg-accent'
                    } ${isCurrent ? 'ring-2 ring-primary/50' : ''}`}
                    aria-label={`会话: ${session.name || '未命名会话'}`}
                    aria-current={isCurrent ? 'true' : 'false'}
                  >
                    <div className="flex items-center justify-between mb-1">
                      <div className="flex items-center gap-2">
                        {/* Checkbox for batch mode */}
                        {isBatchMode && (
                          <input
                            type="checkbox"
                            checked={isSelected}
                            disabled={isCurrent}
                            onChange={() => toggleSessionSelection(session.id, {} as React.MouseEvent)}
                            className="mt-0.5"
                            onClick={(e) => e.stopPropagation()}
                          />
                        )}
                        {/* Current session indicator */}
                        {isCurrent && (
                          <span className="text-primary font-bold" aria-label="当前会话">
                            *
                          </span>
                        )}
                        <div className="font-medium">
                          {session.name || '未命名会话'}
                        </div>
                      </div>
                      <div className="flex items-center gap-2">
                        <div className="text-xs text-muted-foreground">
                          {session.message_count} 条消息
                        </div>
                        {/* Delete button - only show in non-batch mode */}
                        {!isBatchMode && (
                          <button
                            onClick={(e) => handleDeleteSession(session.id, e)}
                            disabled={isDeleting || isCurrent}
                            className={`p-1 rounded transition-colors ${
                              isCurrent
                                ? 'text-muted-foreground/50 cursor-not-allowed'
                                : 'text-muted-foreground hover:text-red-500 hover:bg-red-500/10'
                            }`}
                            aria-label={`删除会话 ${session.name || session.short_id}`}
                            title={isCurrent ? '无法删除当前会话' : '删除会话'}
                          >
                            {isDeleting ? '...' : '🗑'}
                          </button>
                        )}
                      </div>
                    </div>

                    {/* Metadata display */}
                    <div className="text-sm text-muted-foreground flex flex-wrap items-center gap-2">
                      <span className="text-xs font-mono bg-accent px-1 rounded">
                        {session.short_id || session.id.slice(0, 8)}
                      </span>
                      <span>•</span>
                      <span>创建: {session.created_at}</span>
                      {session.updated_at && (
                        <>
                          <span>•</span>
                          <span>更新: {session.updated_at}</span>
                        </>
                      )}
                      {session.project_path && (
                        <>
                          <span>•</span>
                          <span className="truncate max-w-[150px]" title={session.project_path}>
                            项目: {getProjectName(session.project_path)}
                          </span>
                        </>
                      )}
                    </div>

                    {/* Keyboard hint for selected item */}
                    {idx === selectedIndex && !isDeleting && (
                      <div className="mt-2 text-xs text-primary">
                        {isBatchMode
                          ? (session.id === currentSessionId
                              ? '当前会话不可删除'
                              : '按 Space/Enter 选择此会话')
                          : '按 Enter 选择此会话'}
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="border-t p-4 flex justify-between items-center">
          <div className="text-xs text-muted-foreground">
            {filteredSessions.length} 个会话
            {currentSessionId && filteredSessions.some(s => s.id === currentSessionId) && (
              <span className="ml-2 text-primary">(* 当前会话)</span>
            )}
          </div>
          <div className="text-xs text-muted-foreground flex items-center gap-2">
            {isBatchMode ? (
              <>
                <span>
                  <kbd className="px-1 bg-accent rounded">↑↓</kbd>
                  <kbd className="px-1 bg-accent rounded ml-1">j/k</kbd>
                  <span className="ml-1">导航</span>
                </span>
                <span>
                  <kbd className="px-1 bg-accent rounded">Space</kbd>
                  <span className="ml-1">选择</span>
                </span>
                <span>
                  <kbd className="px-1 bg-accent rounded">a</kbd>
                  <span className="ml-1">全选</span>
                </span>
                <span>
                  <kbd className="px-1 bg-accent rounded">d</kbd>
                  <span className="ml-1">删除</span>
                </span>
                <span>
                  <kbd className="px-1 bg-accent rounded">Esc</kbd>
                  <span className="ml-1">退出</span>
                </span>
              </>
            ) : (
              <>
                <span>
                  <kbd className="px-1 bg-accent rounded">↑↓</kbd>
                  <span className="ml-1">或</span>
                  <kbd className="px-1 bg-accent rounded ml-1">j/k</kbd>
                  <span className="ml-1">导航</span>
                </span>
                <span>
                  <kbd className="px-1 bg-accent rounded">Enter</kbd>
                  <span className="ml-1">选择</span>
                </span>
                <span>
                  <kbd className="px-1 bg-accent rounded">Esc</kbd>
                  <span className="ml-1">取消</span>
                </span>
              </>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}