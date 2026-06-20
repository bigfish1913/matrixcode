import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useToastContext } from '../contexts/ToastContext';

interface MemoryEntry {
  key: string;
  value: string;
  timestamp?: number;
  metadata?: Record<string, unknown>;
}

interface MemoryPanelProps {
  onClose: () => void;
}

export function MemoryPanel({ onClose }: MemoryPanelProps) {
  const [memories, setMemories] = useState<MemoryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedMemory, setSelectedMemory] = useState<MemoryEntry | null>(null);
  const [newKey, setNewKey] = useState('');
  const [newValue, setNewValue] = useState('');
  const [isAdding, setIsAdding] = useState(false);

  const toast = useToastContext();

  // Load memories from backend
  useEffect(() => {
    const loadMemories = async () => {
      try {
        setLoading(true);
        const memoryList = await invoke<MemoryEntry[]>('get_memories');
        setMemories(memoryList || []);
      } catch (e) {
        console.error('Failed to load memories:', e);
        toast.addToast({ type: 'error', message: '加载记忆失败' });
      } finally {
        setLoading(false);
      };
    };
    loadMemories();
  }, []);

  // Add new memory
  const handleAddMemory = async () => {
    if (!newKey.trim() || !newValue.trim()) {
      toast.addToast({ type: 'error', message: 'Key和Value都不能为空' });
      return;
    }

    try {
      await invoke('save_memory', { key: newKey, value: newValue });
      toast.addToast({ type: 'success', message: `记忆已保存: ${newKey}` });
      setNewKey('');
      setNewValue('');
      setIsAdding(false);
      // Reload memories
      const memoryList = await invoke<MemoryEntry[]>('get_memories');
      setMemories(memoryList || []);
    } catch (e) {
      console.error('Failed to save memory:', e);
      toast.addToast({ type: 'error', message: '保存记忆失败' });
    }
  };

  // Delete memory
  const handleDeleteMemory = async (key: string) => {
    try {
      await invoke('delete_memory', { key });
      toast.addToast({ type: 'success', message: `记忆已删除: ${key}` });
      setMemories(memories.filter(m => m.key !== key));
      if (selectedMemory?.key === key) {
        setSelectedMemory(null);
      }
    } catch (e) {
      console.error('Failed to delete memory:', e);
      toast.addToast({ type: 'error', message: '删除记忆失败' });
    }
  };

  // Filter memories by search query
  const filteredMemories = memories.filter(m =>
    m.key.toLowerCase().includes(searchQuery.toLowerCase()) ||
    m.value.toLowerCase().includes(searchQuery.toLowerCase())
  );

  // Format timestamp
  const formatTime = (timestamp?: number): string => {
    if (!timestamp) return '';
    const date = new Date(timestamp);
    return date.toLocaleString();
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
      <div className="bg-card border shadow-lg rounded-lg max-w-2xl w-full max-h-[80vh] overflow-hidden flex flex-col">
        {/* Header */}
        <div className="p-4 border-b bg-muted/30">
          <div className="flex items-center justify-between">
            <h3 className="text-lg font-semibold flex items-center gap-2">
              <span>🧠</span>
              <span>Memory Management</span>
            </h3>
            <button
              onClick={onClose}
              className="text-muted-foreground hover:text-foreground p-1 rounded hover:bg-accent transition-colors"
            >
              ✕
            </button>
          </div>
          <p className="text-sm text-muted-foreground mt-1">
            查看和管理持久化记忆
          </p>
        </div>

        {/* Search and Add */}
        <div className="p-3 border-b flex gap-2">
          {/* Search input */}
          <div className="flex-1 relative">
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="搜索记忆..."
              className="w-full px-3 py-1.5 bg-background border rounded text-sm focus:outline-none focus:ring-2 focus:ring-primary"
            />
            {searchQuery && (
              <button
                onClick={() => setSearchQuery('')}
                className="absolute right-2 top-1/2 -translate-y-1/2 text-xs text-muted-foreground hover:text-foreground"
              >
                ✕
              </button>
            )}
          </div>

          {/* Add button */}
          <button
            onClick={() => setIsAdding(!isAdding)}
            className={`px-3 py-1.5 rounded text-sm font-medium transition-colors ${
              isAdding
                ? 'bg-muted text-muted-foreground'
                : 'bg-primary text-primary-foreground hover:bg-primary/90'
            }`}
          >
            {isAdding ? '取消' : '+ 添加'}
          </button>
        </div>

        {/* Add memory form */}
        {isAdding && (
          <div className="p-3 border-b bg-muted/20 animate-slide-in-up">
            <div className="space-y-2">
              <input
                type="text"
                value={newKey}
                onChange={(e) => setNewKey(e.target.value)}
                placeholder="Key (唯一标识)"
                className="w-full px-3 py-1.5 bg-background border rounded text-sm focus:outline-none focus:ring-2 focus:ring-primary"
              />
              <textarea
                value={newValue}
                onChange={(e) => setNewValue(e.target.value)}
                placeholder="Value (记忆内容)"
                rows={3}
                className="w-full px-3 py-1.5 bg-background border rounded text-sm focus:outline-none focus:ring-2 focus:ring-primary resize-none"
              />
              <div className="flex gap-2">
                <button
                  onClick={handleAddMemory}
                  className="px-4 py-1.5 bg-primary text-primary-foreground rounded text-sm font-medium hover:bg-primary/90 transition-colors"
                >
                  保存记忆
                </button>
                <button
                  onClick={() => setIsAdding(false)}
                  className="px-4 py-1.5 bg-muted text-muted-foreground rounded text-sm hover:bg-muted/80 transition-colors"
                >
                  取消
                </button>
              </div>
            </div>
          </div>
        )}

        {/* Memory list */}
        <div className="flex-1 overflow-y-auto">
          {loading ? (
            <div className="text-center text-muted-foreground py-8">
              <div className="animate-spin text-2xl mb-2">⏳</div>
              <span className="text-sm">加载记忆...</span>
            </div>
          ) : filteredMemories.length === 0 ? (
            <div className="text-center text-muted-foreground py-8">
              <p className="text-sm">
                {searchQuery ? '未找到匹配的记忆' : '暂无记忆'}
              </p>
              {!searchQuery && (
                <button
                  onClick={() => setIsAdding(true)}
                  className="mt-2 text-sm text-primary hover:underline"
                >
                  添加第一条记忆
                </button>
              )}
            </div>
          ) : (
            <div className="p-3 space-y-2">
              {filteredMemories.map((memory) => (
                <button
                  key={memory.key}
                  onClick={() => setSelectedMemory(memory)}
                  className={`w-full p-2.5 rounded-lg border transition-all ${
                    selectedMemory?.key === memory.key
                      ? 'border-primary bg-primary/10'
                      : 'border-border hover:border-primary/50 hover:bg-accent/30'
                  }`}
                >
                  <div className="flex items-start gap-2">
                    {/* Memory icon */}
                    <span className="text-lg">🧠</span>

                    {/* Memory info */}
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2">
                        <span className="font-medium text-sm truncate">
                          {memory.key}
                        </span>
                        {memory.timestamp && (
                          <span className="text-xs text-muted-foreground">
                            {formatTime(memory.timestamp)}
                          </span>
                        )}
                      </div>
                      <p className="text-xs text-muted-foreground mt-1 line-clamp-2">
                        {memory.value}
                      </p>
                    </div>

                    {/* Delete button */}
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        handleDeleteMemory(memory.key);
                      }}
                      className="text-xs px-1.5 py-0.5 hover:bg-destructive hover:text-destructive-foreground rounded transition-colors"
                      title="删除记忆"
                    >
                      ✕
                    </button>
                  </div>
                </button>
              ))}
            </div>
          )}
        </div>

        {/* Selected memory detail */}
        {selectedMemory && (
          <div className="p-3 border-t bg-muted/20 animate-slide-in-up">
            <div className="text-sm font-medium mb-2">
              {selectedMemory.key}
            </div>
            <pre className="text-xs bg-background p-2 rounded overflow-auto max-h-40 whitespace-pre-wrap">
              {selectedMemory.value}
            </pre>
            {selectedMemory.metadata && (
              <div className="mt-2 text-xs text-muted-foreground">
                <span>Metadata: {JSON.stringify(selectedMemory.metadata)}</span>
              </div>
            )}
          </div>
        )}

        {/* Footer */}
        <div className="p-3 border-t bg-muted/30">
          <div className="flex justify-between items-center text-xs text-muted-foreground">
            <span>共 {memories.length} 条记忆</span>
            <span>/memory 快捷命令</span>
          </div>
        </div>
      </div>
    </div>
  );
}