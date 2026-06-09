import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

export interface SessionInfo {
  id: string;
  name: string;
  message_count: number;
  created_at: string;
}

interface SessionState {
  sessions: SessionInfo[];
  currentSessionId: string | null;
  loading: boolean;

  loadSessions: () => Promise<void>;
  createSession: (name?: string) => Promise<string>;
  continueLast: () => Promise<string | null>;
  switchSession: (sessionId: string) => Promise<void>;
  resumeSession: (query: string) => Promise<string | null>;
  renameSession: (newName: string) => Promise<void>;
  clearSession: () => Promise<void>;
  setCurrentSession: (id: string | null) => void;
}

export const useSessionStore = create<SessionState>((set, get) => ({
  sessions: [],
  currentSessionId: null,
  loading: false,

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

  setCurrentSession: (id: string | null) => {
    set({ currentSessionId: id });
  },
}));