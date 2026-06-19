import { useEffect } from 'react';
import { create } from 'zustand';

// Persistence configuration
interface PersistenceConfig {
  autoSave: boolean;
  saveInterval: number;
  maxSessionAge: number;
}

// Default config
const DEFAULT_CONFIG: PersistenceConfig = {
  autoSave: true,
  saveInterval: 30000,
  maxSessionAge: 7 * 24 * 60 * 60 * 1000,
};

// Session snapshot
interface SessionSnapshot {
  id: string;
  title: string;
  createdAt: number;
  updatedAt: number;
  messageCount: number;
  preview: string;
}

// Simple persistence state (without Zustand persist middleware)
interface PersistenceState {
  lastSaved: number | null;
  sessions: SessionSnapshot[];
  config: PersistenceConfig;
}

// Persistence store (simple version)
export const usePersistenceStore = create<PersistenceState>(() => ({
  lastSaved: null,
  sessions: [],
  config: DEFAULT_CONFIG,
}));

// Load from localStorage
function loadFromStorage(): void {
  try {
    const stored = localStorage.getItem('matrixcode-sessions');
    if (stored) {
      const sessions = JSON.parse(stored);
      usePersistenceStore.setState({ sessions });
    }
  } catch (e) {
    console.error('Failed to load sessions:', e);
  }
}

// Save to localStorage
function saveToStorage(): void {
  try {
    const state = usePersistenceStore.getState();
    localStorage.setItem('matrixcode-sessions', JSON.stringify(state.sessions.slice(-50)));
    usePersistenceStore.setState({ lastSaved: Date.now() });
  } catch (e) {
    console.error('Failed to save sessions:', e);
  }
}

// Initialize from storage
loadFromStorage();

// Save session snapshot
export function saveSessionSnapshot(
  sessionId: string,
  title: string,
  messageCount: number,
  preview: string
): void {
  const state = usePersistenceStore.getState();
  const now = Date.now();

  const existingIdx = state.sessions.findIndex(s => s.id === sessionId);

  if (existingIdx >= 0) {
    const sessions = [...state.sessions];
    sessions[existingIdx] = {
      ...sessions[existingIdx],
      updatedAt: now,
      messageCount,
      preview,
    };
    usePersistenceStore.setState({ sessions });
  } else {
    const snapshot: SessionSnapshot = {
      id: sessionId,
      title,
      createdAt: now,
      updatedAt: now,
      messageCount,
      preview,
    };
    usePersistenceStore.setState({
      sessions: [...state.sessions, snapshot],
    });
  }

  saveToStorage();
}

// Clean old sessions
export function cleanOldSessions(): void {
  const state = usePersistenceStore.getState();
  const now = Date.now();
  const cutoff = now - state.config.maxSessionAge;

  const recentSessions = state.sessions.filter(s => s.updatedAt > cutoff);
  usePersistenceStore.setState({ sessions: recentSessions });
  saveToStorage();
}

// Persistence manager component
export function PersistenceManager() {
  const config = usePersistenceStore((s) => s.config);

  // Auto-save timer
  useEffect(() => {
    if (!config.autoSave) return;

    const interval = setInterval(() => {
      saveToStorage();
    }, config.saveInterval);

    return () => clearInterval(interval);
  }, [config.autoSave, config.saveInterval]);

  // Clean old sessions on mount
  useEffect(() => {
    cleanOldSessions();
  }, []);

  return null;
}

// Recovery from persisted state
export function recoverFromPersistence(): SessionSnapshot[] {
  return usePersistenceStore.getState().sessions;
}

// Get session statistics
export function getSessionStats() {
  const state = usePersistenceStore.getState();
  return {
    savedSessions: state.sessions.length,
    lastSaved: state.lastSaved,
  };
}

// Persistence status indicator
export function PersistenceIndicator() {
  const lastSaved = usePersistenceStore((s) => s.lastSaved);

  if (!lastSaved) return null;

  const timeSinceSave = Date.now() - lastSaved;
  const seconds = Math.floor(timeSinceSave / 1000);

  let color = 'text-green-500';
  let text = 'Saved';

  if (seconds > 60) {
    color = 'text-yellow-500';
    text = `${Math.floor(seconds / 60)}m ago`;
  }

  return <span className={`text-xs ${color}`}>💾 {text}</span>;
}

// Auto-save hook
export function useAutoSave(
  sessionId: string,
  title: string,
  messageCount: number,
  preview: string
) {
  const config = usePersistenceStore((s) => s.config);

  useEffect(() => {
    if (!config.autoSave) return;
    saveSessionSnapshot(sessionId, title, messageCount, preview);
  }, [sessionId, title, messageCount, preview, config.autoSave]);
}