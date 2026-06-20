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
  searchQuery: string;

  loadSessions: () => Promise<void>;
  createSession: (name?: string) => Promise<string>;
  continueLast: () => Promise<string | null>;
  switchSession: (sessionId: string) => Promise<void>;
  resumeSession: (query: string) => Promise<string | null>;
  renameSession: (newName: string) => Promise<void>;
  clearSession: () => Promise<void>;
  deleteSession: (id: string) => Promise<void>;
  searchSessions: (query: string) => void;
  setCurrentSession: (id: string | null) => void;
}

export const useSessionStore = create<SessionState>((set, get) => ({
  sessions: [],
  currentSessionId: null,
  loading: false,
  searchQuery: '',

  loadSessions: async () => {
    set({ loading: true });
    try {
      const sessions = await invoke<SessionInfo[]>('list_sessions');
      set({ sessions });
      // Also check current session
      const currentId = await invoke<string | null>('current_session');
      set({ currentSessionId: currentId });
    } finally {
      set({ loading: false });
    }
  },

  createSession: async (name?: string) => {
    const id = await invoke<string>('create_session', { name: name ?? null });
    set({ currentSessionId: id });
    await get().loadSessions();
    return id;
  },

  continueLast: async () => {
    const id = await invoke<string | null>('continue_last_session');
    if (id) {
      set({ currentSessionId: id });
      await get().loadSessions();
    }
    return id;
  },

  switchSession: async (sessionId: string) => {
    // Actually switch the backend session
    await invoke('switch_session', { sessionId });
    set({ currentSessionId: sessionId });
    await get().loadSessions();
  },

  resumeSession: async (query: string) => {
    const id = await invoke<string | null>('resume_session', { query });
    if (id) {
      set({ currentSessionId: id });
      await get().loadSessions();
    }
    return id;
  },

  renameSession: async (newName: string) => {
    await invoke('rename_session', { newName });
    await get().loadSessions();
  },

  clearSession: async () => {
    await invoke('clear_session');
  },

  deleteSession: async (id: string) => {
    try {
      await invoke('delete_session', { id });
      // If deleted session was current, clear currentSessionId
      const currentId = get().currentSessionId;
      if (currentId === id) {
        set({ currentSessionId: null });
      }
      await get().loadSessions();
    } catch (e) {
      console.error('Failed to delete session:', e);
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
}));