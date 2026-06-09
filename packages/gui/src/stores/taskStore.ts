import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

export type TaskStatus = 'pending' | 'in_progress' | 'completed' | 'failed' | 'paused';

export interface TaskInfo {
  id: string;
  title: string;
  description?: string;
  status: TaskStatus;
  progress?: number;
  session_id?: string;
  error?: string;
  created_at?: string;
  updated_at?: string;
}

// Agent progress event from backend
interface ProgressEventData {
  task_id?: string;
  message: string;
  percentage: number | null;
}

interface TaskState {
  tasks: TaskInfo[];
  loading: boolean;
  _unlisten: UnlistenFn | null;

  loadTasks: () => Promise<void>;
  cancelTask: (taskId: string) => Promise<void>;
  pauseTask: (taskId: string) => Promise<void>;
  resumeTask: (taskId: string) => Promise<void>;
  startListening: () => Promise<void>;
  stopListening: () => void;
}

export const useTaskStore = create<TaskState>((set, get) => ({
  tasks: [],
  loading: false,
  _unlisten: null,

  loadTasks: async () => {
    set({ loading: true });
    try {
      const tasks = await invoke<TaskInfo[]>('get_tasks');
      set({ tasks });
    } catch (e) {
      // Task endpoint may not be fully implemented yet - silently handle
      console.warn('Failed to load tasks:', e);
    } finally {
      set({ loading: false });
    }
  },

  cancelTask: async (taskId: string) => {
    try {
      await invoke('cancel_task', { taskId });
      set((s) => ({
        tasks: s.tasks.map((t) =>
          t.id === taskId ? { ...t, status: 'failed' as TaskStatus, error: 'Cancelled by user' } : t
        ),
      }));
    } catch (e) {
      console.error('Failed to cancel task:', e);
    }
  },

  pauseTask: async (taskId: string) => {
    try {
      await invoke('pause_task', { taskId });
      set((s) => ({
        tasks: s.tasks.map((t) =>
          t.id === taskId ? { ...t, status: 'paused' as TaskStatus } : t
        ),
      }));
    } catch (e) {
      console.error('Failed to pause task:', e);
    }
  },

  resumeTask: async (taskId: string) => {
    try {
      await invoke('resume_task', { taskId });
      set((s) => ({
        tasks: s.tasks.map((t) =>
          t.id === taskId ? { ...t, status: 'in_progress' as TaskStatus } : t
        ),
      }));
    } catch (e) {
      console.error('Failed to resume task:', e);
    }
  },

  startListening: async () => {
    if (get()._unlisten) return;

    const unlisten = await listen<ProgressEventData>('task-progress', (event) => {
      const { task_id, percentage } = event.payload;
      if (task_id) {
        set((s) => ({
          tasks: s.tasks.map((t) =>
            t.id === task_id
              ? { ...t, progress: percentage != null ? percentage / 100 : undefined }
              : t
          ),
        }));
      }
    });

    set({ _unlisten: unlisten });
  },

  stopListening: () => {
    const unlisten = get()._unlisten;
    if (unlisten) {
      unlisten();
      set({ _unlisten: null });
    }
  },
}));
