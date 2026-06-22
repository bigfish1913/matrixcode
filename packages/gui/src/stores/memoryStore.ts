import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// Types - Matching plan.md MemoryEntry definition
// ============================================================================

/** Memory entry type classification */
export type MemoryType = 'user' | 'feedback' | 'project' | 'reference';

/** Memory entry from backend */
export interface MemoryEntry {
  name: string;            // kebab-case slug
  description: string;     // one-line summary
  type: MemoryType;
  content: string;         // markdown body
  metadata: {
    createdAt: number;
    updatedAt: number;
    tags?: string[];
    links?: string[];      // [[name]] references
  };
}

/** Input for creating new memory */
export interface MemoryEntryInput {
  name: string;
  description: string;
  type: MemoryType;
  content: string;
  tags?: string[];
}

/** Memory summary from backend */
export interface MemorySummary {
  totalEntries: number;
  byType: Record<MemoryType, number>;
  lastUpdated: number;
  summaryText: string;
}

// ============================================================================
// Helper Functions
// ============================================================================

/** Get icon for memory type */
export function getMemoryTypeIcon(type: MemoryType): string {
  switch (type) {
    case 'user': return '👤';
    case 'feedback': return '📝';
    case 'project': return '📁';
    case 'reference': return '📚';
    default: return '🧠';
  }
}

/** Get color class for memory type */
export function getMemoryTypeColor(type: MemoryType): string {
  switch (type) {
    case 'user': return 'text-blue-500';
    case 'feedback': return 'text-yellow-500';
    case 'project': return 'text-green-500';
    case 'reference': return 'text-purple-500';
    default: return 'text-gray-500';
  }
}

/** Get label for memory type */
export function getMemoryTypeLabel(type: MemoryType): string {
  switch (type) {
    case 'user': return '用户记忆';
    case 'feedback': return '反馈记录';
    case 'project': return '项目信息';
    case 'reference': return '参考资料';
    default: return '未知';
  }
}

/** Format timestamp to readable string - handles undefined/null/invalid values */
export function formatMemoryTime(timestamp: number | undefined | null): string {
  if (timestamp === undefined || timestamp === null || timestamp === 0) {
    return '未知时间';
  }
  try {
    const date = new Date(timestamp);
    if (isNaN(date.getTime())) {
      return '未知时间';
    }
    return date.toLocaleString('zh-CN', {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
    });
  } catch {
    return '未知时间';
  }
}

// ============================================================================
// Store State Interface
// ============================================================================

interface MemoryState {
  // Memory entries
  memories: MemoryEntry[];

  // Memory summary
  summary: MemorySummary | null;

  // Search query
  searchQuery: string;

  // Type filter
  typeFilter: MemoryType | 'all';

  // Loading state
  loading: boolean;

  // Selected memory for detail view
  selectedMemory: MemoryEntry | null;

  // Editing state
  editingMemory: MemoryEntry | null;
  isEditing: boolean;

  // Actions
  loadMemory: () => Promise<void>;
  searchMemory: (query: string) => Promise<void>;
  addMemory: (entry: MemoryEntryInput) => Promise<void>;
  deleteMemory: (name: string) => Promise<void>;
  updateMemory: (name: string, content: string) => Promise<void>;
  setSearchQuery: (query: string) => void;
  setTypeFilter: (type: MemoryType | 'all') => void;
  setSelectedMemory: (memory: MemoryEntry | null) => void;
  startEditing: (memory: MemoryEntry) => void;
  stopEditing: () => void;
  clearFilters: () => void;
}

// ============================================================================
// Store Implementation
// ============================================================================

export const useMemoryStore = create<MemoryState>((set, get) => ({
  memories: [],
  summary: null,
  searchQuery: '',
  typeFilter: 'all',
  loading: false,
  selectedMemory: null,
  editingMemory: null,
  isEditing: false,

  // Load all memories from backend
  loadMemory: async () => {
    set({ loading: true });
    try {
      const result = await invoke<{ entries: MemoryEntry[]; summary: MemorySummary } | null>('load_memory');
      if (result && result.entries && Array.isArray(result.entries)) {
        set({
          memories: result.entries,
          summary: result.summary || null,
          loading: false,
        });
      } else {
        // Invalid result structure or null
        set({ memories: [], summary: null, loading: false });
      }
    } catch (e) {
      console.error('加载记忆失败:', e);
      set({ loading: false });
    }
  },

  // Search memories
  searchMemory: async (query: string) => {
    set({ loading: true, searchQuery: query });
    try {
      if (query.trim() === '') {
        // Empty query - load all memories
        await get().loadMemory();
        return;
      }
      const entries = await invoke<MemoryEntry[]>('search_memory', { query });
      set({ memories: entries, loading: false });
    } catch (e) {
      console.error('搜索记忆失败:', e);
      set({ loading: false });
    }
  },

  // Add new memory
  addMemory: async (entry: MemoryEntryInput) => {
    set({ loading: true });
    try {
      await invoke('add_memory', { entry });
      // Reload memories after adding
      await get().loadMemory();
    } catch (e) {
      console.error('添加记忆失败:', e);
      set({ loading: false });
      throw e;
    }
  },

  // Delete memory by name
  deleteMemory: async (name: string) => {
    try {
      await invoke('delete_memory', { name });
      set((s) => ({
        memories: s.memories.filter(m => m.name !== name),
        selectedMemory: s.selectedMemory?.name === name ? null : s.selectedMemory,
      }));
      // Update summary
      await get().loadMemory();
    } catch (e) {
      console.error('删除记忆失败:', e);
      throw e;
    }
  },

  // Update memory content
  updateMemory: async (name: string, content: string) => {
    set({ loading: true });
    try {
      await invoke('update_memory', { name, content });
      // Update local state
      set((s) => ({
        memories: s.memories.map(m =>
          m.name === name
            ? { ...m, content, metadata: { ...m.metadata, updatedAt: Date.now() } }
            : m
        ),
        editingMemory: null,
        isEditing: false,
        loading: false,
      }));
    } catch (e) {
      console.error('更新记忆失败:', e);
      set({ loading: false });
      throw e;
    }
  },

  // Set search query (local filter)
  setSearchQuery: (query: string) => {
    set({ searchQuery: query });
  },

  // Set type filter
  setTypeFilter: (type: MemoryType | 'all') => {
    set({ typeFilter: type });
  },

  // Set selected memory for detail view
  setSelectedMemory: (memory: MemoryEntry | null) => {
    set({ selectedMemory: memory });
  },

  // Start editing a memory
  startEditing: (memory: MemoryEntry) => {
    set({ editingMemory: memory, isEditing: true });
  },

  // Stop editing
  stopEditing: () => {
    set({ editingMemory: null, isEditing: false });
  },

  // Clear all filters
  clearFilters: () => {
    set({ searchQuery: '', typeFilter: 'all' });
  },
}));

// ============================================================================
// Filtered Memories Helper (used in components)
// ============================================================================

/** Get filtered memories based on search query and type filter */
export function getFilteredMemories(state: MemoryState): MemoryEntry[] {
  let filtered = state.memories;

  // Apply type filter
  if (state.typeFilter !== 'all') {
    filtered = filtered.filter(m => m.type === state.typeFilter);
  }

  // Apply search query (local filter for instant feedback)
  if (state.searchQuery.trim() !== '') {
    const queryLower = state.searchQuery.toLowerCase();
    filtered = filtered.filter(m =>
      m.name.toLowerCase().includes(queryLower) ||
      m.description.toLowerCase().includes(queryLower) ||
      m.content.toLowerCase().includes(queryLower) ||
      m.metadata.tags?.some(t => t.toLowerCase().includes(queryLower))
    );
  }

  // Sort by updatedAt (most recent first)
  filtered.sort((a, b) => b.metadata.updatedAt - a.metadata.updatedAt);

  return filtered;
}

// ============================================================================
// Default Export
// ============================================================================

export default useMemoryStore;