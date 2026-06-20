import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

// ============================================================================
// Types - Matching Rust backend approval.rs
// ============================================================================

/** Approval mode controlling when the user is prompted */
export type ApprovalMode = 'ask' | 'auto' | 'strict';

/** Risk level assigned to each tool operation */
export type RiskLevel = 'safe' | 'mutating' | 'dangerous';

/** User's response to an approval prompt */
export type ApprovalAnswer = 'yes' | 'no' | 'abort';

/** Approval request from backend */
export interface ApprovalRequest {
  id: string;
  tool_name: string;
  risk_level: RiskLevel;
  summary: string;
  input: Record<string, unknown>;
  timestamp: number;
}

/** Approval history record */
export interface ApprovalRecord {
  id: string;
  request: ApprovalRequest;
  answer: ApprovalAnswer;
  reason?: string;
  timestamp: number;
  autoApproved: boolean;
}

/** Approval statistics */
export interface ApprovalStats {
  approved: number;
  rejected: number;
  autoApproved: number;
  aborted: number;
  total: number;
}

// ============================================================================
// Helper Functions
// ============================================================================

/** Get icon for risk level */
export function getRiskLevelIcon(level: RiskLevel): string {
  switch (level) {
    case 'safe': return 'information_source';
    case 'mutating': return 'pencil';
    case 'dangerous': return 'warning';
    default: return 'circle';
  }
}

/** Get color class for risk level */
export function getRiskLevelColor(level: RiskLevel): string {
  switch (level) {
    case 'safe': return 'text-blue-500';
    case 'mutating': return 'text-yellow-500';
    case 'dangerous': return 'text-red-500';
    default: return 'text-gray-500';
  }
}

/** Get label for approval mode */
export function getApprovalModeLabel(mode: ApprovalMode): string {
  switch (mode) {
    case 'ask': return 'Ask';
    case 'auto': return 'Auto';
    case 'strict': return 'Strict';
    default: return 'Unknown';
  }
}

/** Get description for approval mode */
export function getApprovalModeDescription(mode: ApprovalMode): string {
  switch (mode) {
    case 'ask': return '每次操作都需要确认';
    case 'auto': return '自动执行安全操作';
    case 'strict': return '严格确认所有操作';
    default: return '';
  }
}

/** Get icon for approval mode */
export function getApprovalModeIcon(mode: ApprovalMode): string {
  switch (mode) {
    case 'ask': return 'question';
    case 'auto': return 'zap';
    case 'strict': return 'lock';
    default: return 'circle';
  }
}

/** Get color class for approval mode */
export function getApprovalModeColor(mode: ApprovalMode): string {
  switch (mode) {
    case 'ask': return 'text-gray-500';
    case 'auto': return 'text-green-600';
    case 'strict': return 'text-red-600';
    default: return 'text-gray-500';
  }
}

// ============================================================================
// Store State Interface
// ============================================================================

interface ApprovalState {
  // Current mode
  mode: ApprovalMode;

  // Pending approval requests queue
  pendingApprovals: ApprovalRequest[];

  // Approval history records
  approvalHistory: ApprovalRecord[];

  // Statistics
  stats: ApprovalStats;

  // Loading state
  loading: boolean;

  // Current approval request being displayed
  currentApproval: ApprovalRequest | null;

  // Event listener cleanup
  _unlisten: UnlistenFn | null;

  // Actions
  setMode: (mode: ApprovalMode) => Promise<void>;
  loadMode: () => Promise<void>;
  loadHistory: () => Promise<void>;
  approve: (id: string) => Promise<void>;
  reject: (id: string, reason?: string) => Promise<void>;
  abort: () => Promise<void>;
  clearHistory: () => void;
  startListening: () => Promise<void>;
  stopListening: () => void;
  addApprovalRequest: (request: ApprovalRequest) => void;
  removeApprovalRequest: (id: string) => void;
  setCurrentApproval: (request: ApprovalRequest | null) => void;
}

// ============================================================================
// Store Implementation
// ============================================================================

export const useApprovalStore = create<ApprovalState>((set, get) => ({
  mode: 'ask',
  pendingApprovals: [],
  approvalHistory: [],
  stats: { approved: 0, rejected: 0, autoApproved: 0, aborted: 0, total: 0 },
  loading: false,
  currentApproval: null,
  _unlisten: null,

  // Set approval mode
  setMode: async (mode: ApprovalMode) => {
    set({ loading: true });
    try {
      await invoke('set_approve_mode', { mode });
      set({ mode, loading: false });

      // Add to history as auto-approved if switching to auto mode
      if (mode === 'auto') {
        set((s) => ({
          stats: {
            ...s.stats,
            autoApproved: s.stats.autoApproved + 1,
          },
        }));
      }
    } catch (e) {
      console.error('Failed to set approval mode:', e);
      set({ loading: false });
    }
  },

  // Load current approval mode from backend
  loadMode: async () => {
    try {
      const mode = await invoke<ApprovalMode>('get_approve_mode');
      set({ mode: mode || 'ask' });
    } catch (e) {
      console.error('Failed to load approval mode:', e);
    }
  },

  // Load approval history from backend
  loadHistory: async () => {
    set({ loading: true });
    try {
      const history = await invoke<ApprovalRecord[]>('get_approval_history');
      const stats = calculateStats(history);
      set({ approvalHistory: history, stats, loading: false });
    } catch (e) {
      console.error('Failed to load approval history:', e);
      set({ loading: false });
    }
  },

  // Approve a pending request
  approve: async (id: string) => {
    const request = get().pendingApprovals.find(r => r.id === id);
    if (!request) return;

    try {
      await invoke('approve_action', { id });

      // Add to history
      const record: ApprovalRecord = {
        id: `record-${Date.now()}`,
        request,
        answer: 'yes',
        timestamp: Date.now(),
        autoApproved: false,
      };

      set((s) => ({
        pendingApprovals: s.pendingApprovals.filter(r => r.id !== id),
        approvalHistory: [...s.approvalHistory, record],
        stats: {
          ...s.stats,
          approved: s.stats.approved + 1,
          total: s.stats.total + 1,
        },
        currentApproval: null,
      }));
    } catch (e) {
      console.error('Failed to approve action:', e);
    }
  },

  // Reject a pending request
  reject: async (id: string, reason?: string) => {
    const request = get().pendingApprovals.find(r => r.id === id);
    if (!request) return;

    try {
      await invoke('reject_action', { id, reason });

      // Add to history
      const record: ApprovalRecord = {
        id: `record-${Date.now()}`,
        request,
        answer: 'no',
        reason,
        timestamp: Date.now(),
        autoApproved: false,
      };

      set((s) => ({
        pendingApprovals: s.pendingApprovals.filter(r => r.id !== id),
        approvalHistory: [...s.approvalHistory, record],
        stats: {
          ...s.stats,
          rejected: s.stats.rejected + 1,
          total: s.stats.total + 1,
        },
        currentApproval: null,
      }));
    } catch (e) {
      console.error('Failed to reject action:', e);
    }
  },

  // Abort current turn
  abort: async () => {
    const request = get().currentApproval;
    if (!request) return;

    try {
      await invoke('reject_action', { id: request.id, reason: 'abort' });

      // Add to history
      const record: ApprovalRecord = {
        id: `record-${Date.now()}`,
        request,
        answer: 'abort',
        reason: 'User aborted the turn',
        timestamp: Date.now(),
        autoApproved: false,
      };

      set((s) => ({
        pendingApprovals: [],
        approvalHistory: [...s.approvalHistory, record],
        stats: {
          ...s.stats,
          aborted: s.stats.aborted + 1,
          total: s.stats.total + 1,
        },
        currentApproval: null,
      }));
    } catch (e) {
      console.error('Failed to abort:', e);
    }
  },

  // Clear history (local only)
  clearHistory: () => {
    set({ approvalHistory: [], stats: { approved: 0, rejected: 0, autoApproved: 0, aborted: 0, total: 0 } });
  },

  // Start listening for approval events from backend
  startListening: async () => {
    if (get()._unlisten) return;

    try {
      const unlisten = await listen<{ approval: ApprovalRequest }>('approval-request', (event) => {
        const request = event.payload.approval;
        set((s) => ({
          pendingApprovals: [...s.pendingApprovals, request],
          currentApproval: s.currentApproval || request, // Show first request if none selected
        }));
      });

      set({ _unlisten: unlisten });
    } catch (e) {
      console.error('Failed to start approval listener:', e);
    }
  },

  // Stop listening
  stopListening: () => {
    const unlisten = get()._unlisten;
    if (unlisten) {
      unlisten();
      set({ _unlisten: null });
    }
  },

  // Add approval request (for local testing)
  addApprovalRequest: (request: ApprovalRequest) => {
    set((s) => ({
      pendingApprovals: [...s.pendingApprovals, request],
      currentApproval: s.currentApproval || request,
    }));
  },

  // Remove approval request
  removeApprovalRequest: (id: string) => {
    set((s) => ({
      pendingApprovals: s.pendingApprovals.filter(r => r.id !== id),
      currentApproval: s.pendingApprovals.length > 1
        ? s.pendingApprovals.find(r => r.id !== id) || null
        : null,
    }));
  },

  // Set current approval being displayed
  setCurrentApproval: (request: ApprovalRequest | null) => {
    set({ currentApproval: request });
  },
}));

// ============================================================================
// Helper Functions for Stats
// ============================================================================

function calculateStats(history: ApprovalRecord[]): ApprovalStats {
  const stats: ApprovalStats = {
    approved: 0,
    rejected: 0,
    autoApproved: 0,
    aborted: 0,
    total: history.length,
  };

  for (const record of history) {
    if (record.autoApproved) {
      stats.autoApproved++;
    } else {
      switch (record.answer) {
        case 'yes':
          stats.approved++;
          break;
        case 'no':
          stats.rejected++;
          break;
        case 'abort':
          stats.aborted++;
          break;
      }
    }
  }

  return stats;
}

// ============================================================================
// Default Export
// ============================================================================

export default useApprovalStore;