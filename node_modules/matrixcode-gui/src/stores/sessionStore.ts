import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

export interface SessionInfo {
  id: string;
  name: string;
  message_count: number;
  created_at: string;
  project_path?: string;
  updated_at?: string;
  short_id?: string;
}

interface SessionState {
  sessions: SessionInfo[];
  currentSessionId: string | null;
  loading: boolean;
  error: string | null;
  searchQuery: string;
  selectedIds: Set<string>;
  selectionMode: boolean;

  loadSessions: () => Promise<void>;
  createSession: (name?: string) => Promise<string>;
  continueLast: () => Promise<string | null>;
  switchSession: (sessionId: string) => Promise<void>;
  resumeSession: (query: string) => Promise<string | null>;
  renameSession: (newName: string) => Promise<void>;
  clearSession: () => Promise<void>;
  deleteSession: (id: string) => Promise<void>;
  batchDeleteSessions: (ids: string[]) => Promise<number>;
  searchSessions: (query: string) => void;
  setCurrentSession: (id: string | null) => void;
  clearError: () => void;
  // Selection mode methods
  toggleSelectionMode: () => void;
  toggleSelection: (id: string) => void;
  selectAll: () => void;
  clearSelection: () => void;
}

export const useSessionStore = create<SessionState>((set, get) => ({
  sessions: [],
  currentSessionId: null,
  loading: false,
  error: null,
  searchQuery: '',
  selectedIds: new Set<string>(),
  selectionMode: false,

  loadSessions: async () => {
    set({ loading: true, error: null });
    try {
      const sessions = await invoke<SessionInfo[]>('list_sessions');
      set({ sessions });
      // Also check current session
      const currentId = await invoke<string | null>('current_session');
      set({ currentSessionId: currentId, loading: false });
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : '加载会话失败';
      set({ loading: false, error: errorMsg });
    }
  },

  createSession: async (name?: string) => {
    set({ loading: true, error: null });
    try {
      const id = await invoke<string>('create_session', { name: name ?? null });
      set({ currentSessionId: id, loading: false });
      await get().loadSessions();
      return id;
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : '创建会话失败';
      set({ loading: false, error: errorMsg });
      throw e;
    }
  },

  continueLast: async () => {
    set({ loading: true, error: null });
    try {
      const id = await invoke<string | null>('continue_last_session');
      if (id) {
        set({ currentSessionId: id, loading: false });
        await get().loadSessions();
      } else {
        set({ loading: false });
      }
      return id;
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : '恢复会话失败';
      set({ loading: false, error: errorMsg });
      return null;
    }
  },

  switchSession: async (sessionId: string) => {
    set({ loading: true, error: null });
    try {
      // Actually switch the backend session
      await invoke('switch_session', { sessionId });
      set({ currentSessionId: sessionId, loading: false });
      await get().loadSessions();
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : '切换会话失败';
      set({ loading: false, error: errorMsg });
      throw e;
    }
  },

  resumeSession: async (query: string) => {
    set({ loading: true, error: null });
    try {
      const id = await invoke<string | null>('resume_session', { query });
      if (id) {
        set({ currentSessionId: id, loading: false });
        await get().loadSessions();
      } else {
        set({ loading: false });
      }
      return id;
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : '恢复会话失败';
      set({ loading: false, error: errorMsg });
      return null;
    }
  },

  renameSession: async (newName: string) => {
    try {
      await invoke('rename_session', { newName });
      await get().loadSessions();
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : '重命名失败';
      set({ error: errorMsg });
      throw e;
    }
  },

  clearSession: async () => {
    try {
      await invoke('clear_session');
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : '清空会话失败';
      set({ error: errorMsg });
      throw e;
    }
  },

  deleteSession: async (id: string) => {
    set({ loading: true, error: null });
    try {
      await invoke('delete_session', { id });
      // If deleted session was current, clear currentSessionId
      const currentId = get().currentSessionId;
      if (currentId === id) {
        set({ currentSessionId: null });
      }
      set({ loading: false });
      await get().loadSessions();
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : '删除会话失败';
      set({ loading: false, error: errorMsg });
      throw e;
    }
  },

  batchDeleteSessions: async (ids: string[]) => {
    if (ids.length === 0) return 0;
    set({ loading: true, error: null });
    try {
      const deletedCount = await invoke<number>('batch_delete_sessions', { ids });
      // If any deleted session was current, clear currentSessionId
      const currentId = get().currentSessionId;
      if (currentId && ids.includes(currentId)) {
        set({ currentSessionId: null });
      }
      // Clear selection
      set({ loading: false, selectedIds: new Set(), selectionMode: false });
      await get().loadSessions();
      return deletedCount;
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : '批量删除会话失败';
      set({ loading: false, error: errorMsg });
      throw e;
    }
  },

  searchSessions: (query: string) => {
    set({ searchQuery: query });
    // Filter is done in the component, not here
  },

  setCurrentSession: (id: string | null) => {
    set({ currentSessionId: id });
  },

  clearError: () => {
    set({ error: null });
  },

  // Selection mode methods
  toggleSelectionMode: () => {
    const currentMode = get().selectionMode;
    set({
      selectionMode: !currentMode,
      selectedIds: new Set(),
    });
  },

  toggleSelection: (id: string) => {
    const selectedIds = new Set(get().selectedIds);
    if (selectedIds.has(id)) {
      selectedIds.delete(id);
    } else {
      selectedIds.add(id);
    }
    set({ selectedIds });
  },

  selectAll: () => {
    const allIds = new Set(get().sessions.map(s => s.id));
    set({ selectedIds: allIds });
  },

  clearSelection: () => {
    set({ selectedIds: new Set() });
  },
}));