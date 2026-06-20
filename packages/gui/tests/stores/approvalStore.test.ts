import { describe, it, expect, beforeEach, vi } from 'vitest';
import { act } from '@testing-library/react';
import {
  useApprovalStore,
  getRiskLevelIcon,
  getRiskLevelColor,
  getApprovalModeLabel,
  getApprovalModeDescription,
  getApprovalModeIcon,
  getApprovalModeColor,
  type ApprovalMode,
  type RiskLevel,
  type ApprovalRequest,
  type ApprovalRecord,
} from '../../src/stores/approvalStore';
import { mockInvoke, mockListen, mockUnlisten } from '../setup';

describe('approvalStore', () => {
  beforeEach(() => {
    // Reset store state before each test
    useApprovalStore.setState({
      mode: 'ask',
      pendingApprovals: [],
      approvalHistory: [],
      stats: { approved: 0, rejected: 0, autoApproved: 0, aborted: 0, total: 0 },
      loading: false,
      currentApproval: null,
      _unlisten: null,
    });
  });

  // ============================================================================
  // Helper Functions Tests
  // ============================================================================

  describe('helper functions', () => {
    describe('getRiskLevelIcon', () => {
      it('should return correct icon for each risk level', () => {
        expect(getRiskLevelIcon('safe')).toBe('ℹ️');
        expect(getRiskLevelIcon('mutating')).toBe('✏️');
        expect(getRiskLevelIcon('dangerous')).toBe('⚠️');
      });

      it('should return default icon for unknown risk level', () => {
        expect(getRiskLevelIcon('unknown' as RiskLevel)).toBe('●');
      });
    });

    describe('getRiskLevelColor', () => {
      it('should return correct color class for each risk level', () => {
        expect(getRiskLevelColor('safe')).toBe('text-blue-500');
        expect(getRiskLevelColor('mutating')).toBe('text-yellow-500');
        expect(getRiskLevelColor('dangerous')).toBe('text-red-500');
      });

      it('should return default color for unknown risk level', () => {
        expect(getRiskLevelColor('unknown' as RiskLevel)).toBe('text-gray-500');
      });
    });

    describe('getApprovalModeLabel', () => {
      it('should return correct label for each mode', () => {
        expect(getApprovalModeLabel('ask')).toBe('询问');
        expect(getApprovalModeLabel('auto')).toBe('自动');
        expect(getApprovalModeLabel('strict')).toBe('严格');
      });

      it('should return Unknown for unknown mode', () => {
        expect(getApprovalModeLabel('unknown' as ApprovalMode)).toBe('未知');
      });
    });

    describe('getApprovalModeDescription', () => {
      it('should return correct description for each mode', () => {
        expect(getApprovalModeDescription('ask')).toBe('每次操作都需要确认');
        expect(getApprovalModeDescription('auto')).toBe('自动执行安全操作');
        expect(getApprovalModeDescription('strict')).toBe('严格确认所有操作');
      });

      it('should return empty string for unknown mode', () => {
        expect(getApprovalModeDescription('unknown' as ApprovalMode)).toBe('');
      });
    });

    describe('getApprovalModeIcon', () => {
      it('should return correct icon for each mode', () => {
        expect(getApprovalModeIcon('ask')).toBe('❓');
        expect(getApprovalModeIcon('auto')).toBe('⚡');
        expect(getApprovalModeIcon('strict')).toBe('🔒');
      });

      it('should return default icon for unknown mode', () => {
        expect(getApprovalModeIcon('unknown' as ApprovalMode)).toBe('●');
      });
    });

    describe('getApprovalModeColor', () => {
      it('should return correct color class for each mode', () => {
        expect(getApprovalModeColor('ask')).toBe('text-gray-500');
        expect(getApprovalModeColor('auto')).toBe('text-green-600');
        expect(getApprovalModeColor('strict')).toBe('text-red-600');
      });

      it('should return default color for unknown mode', () => {
        expect(getApprovalModeColor('unknown' as ApprovalMode)).toBe('text-gray-500');
      });
    });
  });

  // ============================================================================
  // Store State Tests
  // ============================================================================

  describe('initial state', () => {
    it('should have correct initial state', () => {
      const state = useApprovalStore.getState();

      expect(state.mode).toBe('ask');
      expect(state.pendingApprovals).toEqual([]);
      expect(state.approvalHistory).toEqual([]);
      expect(state.stats).toEqual({
        approved: 0,
        rejected: 0,
        autoApproved: 0,
        aborted: 0,
        total: 0,
      });
      expect(state.loading).toBe(false);
      expect(state.currentApproval).toBeNull();
      expect(state._unlisten).toBeNull();
    });
  });

  // ============================================================================
  // Mode Management Tests
  // ============================================================================

  describe('setMode', () => {
    it('should set mode to auto', async () => {
      mockInvoke.mockResolvedValueOnce(undefined);

      await act(async () => {
        await useApprovalStore.getState().setMode('auto');
      });

      expect(mockInvoke).toHaveBeenCalledWith('set_approve_mode', { mode: 'auto' });
      expect(useApprovalStore.getState().mode).toBe('auto');
      expect(useApprovalStore.getState().loading).toBe(false);
    });

    it('should set mode to strict', async () => {
      mockInvoke.mockResolvedValueOnce(undefined);

      await act(async () => {
        await useApprovalStore.getState().setMode('strict');
      });

      expect(mockInvoke).toHaveBeenCalledWith('set_approve_mode', { mode: 'strict' });
      expect(useApprovalStore.getState().mode).toBe('strict');
    });

    it('should set mode to ask', async () => {
      mockInvoke.mockResolvedValueOnce(undefined);

      await act(async () => {
        await useApprovalStore.getState().setMode('ask');
      });

      expect(mockInvoke).toHaveBeenCalledWith('set_approve_mode', { mode: 'ask' });
      expect(useApprovalStore.getState().mode).toBe('ask');
    });

    it('should increment autoApproved stats when switching to auto mode', async () => {
      mockInvoke.mockResolvedValueOnce(undefined);

      await act(async () => {
        await useApprovalStore.getState().setMode('auto');
      });

      const state = useApprovalStore.getState();
      expect(state.stats.autoApproved).toBe(1);
    });

    it('should not increment autoApproved stats when switching to non-auto mode', async () => {
      mockInvoke.mockResolvedValueOnce(undefined);

      await act(async () => {
        await useApprovalStore.getState().setMode('strict');
      });

      const state = useApprovalStore.getState();
      expect(state.stats.autoApproved).toBe(0);
    });

    it('should handle errors gracefully', async () => {
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
      mockInvoke.mockRejectedValueOnce(new Error('Network error'));

      await act(async () => {
        await useApprovalStore.getState().setMode('auto');
      });

      expect(consoleSpy).toHaveBeenCalledWith('Failed to set approval mode:', expect.any(Error));
      expect(useApprovalStore.getState().mode).toBe('ask'); // Should remain unchanged
      expect(useApprovalStore.getState().loading).toBe(false);

      consoleSpy.mockRestore();
    });

    it('should set loading state during operation', async () => {
      let resolvePromise: () => void;
      mockInvoke.mockImplementation(() => new Promise<void>((resolve) => {
        resolvePromise = resolve;
      }));

      const promise = act(async () => {
        await useApprovalStore.getState().setMode('auto');
      });

      // Check loading state before promise resolves
      expect(useApprovalStore.getState().loading).toBe(true);

      // Resolve the promise
      await act(async () => {
        resolvePromise!();
        await promise;
      });

      expect(useApprovalStore.getState().loading).toBe(false);
    });
  });

  describe('loadMode', () => {
    it('should load mode from backend', async () => {
      mockInvoke.mockResolvedValueOnce('strict');

      await act(async () => {
        await useApprovalStore.getState().loadMode();
      });

      expect(mockInvoke).toHaveBeenCalledWith('get_approve_mode');
      expect(useApprovalStore.getState().mode).toBe('strict');
    });

    it('should default to ask mode when backend returns null', async () => {
      mockInvoke.mockResolvedValueOnce(null);

      await act(async () => {
        await useApprovalStore.getState().loadMode();
      });

      expect(useApprovalStore.getState().mode).toBe('ask');
    });

    it('should handle errors gracefully', async () => {
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
      mockInvoke.mockRejectedValueOnce(new Error('Network error'));

      await act(async () => {
        await useApprovalStore.getState().loadMode();
      });

      expect(consoleSpy).toHaveBeenCalledWith('Failed to load approval mode:', expect.any(Error));
      expect(useApprovalStore.getState().mode).toBe('ask'); // Should remain at default

      consoleSpy.mockRestore();
    });
  });

  // ============================================================================
  // History Management Tests
  // ============================================================================

  describe('loadHistory', () => {
    it('should load history from backend', async () => {
      const mockHistory: ApprovalRecord[] = [
        {
          id: 'record-1',
          request: {
            id: 'req-1',
            tool_name: 'Bash',
            risk_level: 'safe',
            summary: 'List files',
            input: { path: '/home' },
            timestamp: Date.now(),
          },
          answer: 'yes',
          timestamp: Date.now(),
          autoApproved: false,
        },
        {
          id: 'record-2',
          request: {
            id: 'req-2',
            tool_name: 'Edit',
            risk_level: 'mutating',
            summary: 'Edit file',
            input: { file_path: '/home/test.ts' },
            timestamp: Date.now(),
          },
          answer: 'no',
          reason: 'User rejected',
          timestamp: Date.now(),
          autoApproved: false,
        },
      ];
      mockInvoke.mockResolvedValueOnce(mockHistory);

      await act(async () => {
        await useApprovalStore.getState().loadHistory();
      });

      expect(mockInvoke).toHaveBeenCalledWith('get_approval_history');
      const state = useApprovalStore.getState();
      expect(state.approvalHistory).toEqual(mockHistory);
      expect(state.stats.approved).toBe(1);
      expect(state.stats.rejected).toBe(1);
      expect(state.stats.total).toBe(2);
    });

    it('should calculate stats correctly for auto-approved records', async () => {
      const mockHistory: ApprovalRecord[] = [
        {
          id: 'record-1',
          request: {
            id: 'req-1',
            tool_name: 'Read',
            risk_level: 'safe',
            summary: 'Read file',
            input: {},
            timestamp: Date.now(),
          },
          answer: 'yes',
          timestamp: Date.now(),
          autoApproved: true,
        },
      ];
      mockInvoke.mockResolvedValueOnce(mockHistory);

      await act(async () => {
        await useApprovalStore.getState().loadHistory();
      });

      const state = useApprovalStore.getState();
      expect(state.stats.autoApproved).toBe(1);
      expect(state.stats.approved).toBe(0); // Auto-approved doesn't count as regular approved
    });

    it('should handle empty history', async () => {
      mockInvoke.mockResolvedValueOnce([]);

      await act(async () => {
        await useApprovalStore.getState().loadHistory();
      });

      const state = useApprovalStore.getState();
      expect(state.approvalHistory).toEqual([]);
      expect(state.stats).toEqual({
        approved: 0,
        rejected: 0,
        autoApproved: 0,
        aborted: 0,
        total: 0,
      });
    });

    it('should handle errors gracefully', async () => {
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
      mockInvoke.mockRejectedValueOnce(new Error('Network error'));

      await act(async () => {
        await useApprovalStore.getState().loadHistory();
      });

      expect(consoleSpy).toHaveBeenCalledWith('Failed to load approval history:', expect.any(Error));
      expect(useApprovalStore.getState().loading).toBe(false);

      consoleSpy.mockRestore();
    });
  });

  describe('clearHistory', () => {
    it('should clear history and reset stats', () => {
      // Set up some initial state
      useApprovalStore.setState({
        approvalHistory: [
          {
            id: 'record-1',
            request: {
              id: 'req-1',
              tool_name: 'Bash',
              risk_level: 'safe',
              summary: 'Test',
              input: {},
              timestamp: Date.now(),
            },
            answer: 'yes',
            timestamp: Date.now(),
            autoApproved: false,
          },
        ],
        stats: { approved: 1, rejected: 0, autoApproved: 0, aborted: 0, total: 1 },
      });

      act(() => {
        useApprovalStore.getState().clearHistory();
      });

      const state = useApprovalStore.getState();
      expect(state.approvalHistory).toEqual([]);
      expect(state.stats).toEqual({
        approved: 0,
        rejected: 0,
        autoApproved: 0,
        aborted: 0,
        total: 0,
      });
    });
  });

  // ============================================================================
  // Approval Actions Tests
  // ============================================================================

  describe('approve', () => {
    const mockRequest: ApprovalRequest = {
      id: 'req-1',
      tool_name: 'Bash',
      risk_level: 'safe',
      summary: 'List files',
      input: { path: '/home' },
      timestamp: Date.now(),
    };

    it('should approve a pending request', async () => {
      mockInvoke.mockResolvedValueOnce(undefined);
      useApprovalStore.setState({ pendingApprovals: [mockRequest], currentApproval: mockRequest });

      await act(async () => {
        await useApprovalStore.getState().approve('req-1');
      });

      expect(mockInvoke).toHaveBeenCalledWith('approve_action', { id: 'req-1' });

      const state = useApprovalStore.getState();
      expect(state.pendingApprovals).toEqual([]);
      expect(state.approvalHistory).toHaveLength(1);
      expect(state.approvalHistory[0].answer).toBe('yes');
      expect(state.approvalHistory[0].autoApproved).toBe(false);
      expect(state.stats.approved).toBe(1);
      expect(state.stats.total).toBe(1);
      expect(state.currentApproval).toBeNull();
    });

    it('should do nothing if request not found', async () => {
      useApprovalStore.setState({ pendingApprovals: [mockRequest] });

      await act(async () => {
        await useApprovalStore.getState().approve('non-existent');
      });

      expect(mockInvoke).not.toHaveBeenCalled();
      expect(useApprovalStore.getState().stats.approved).toBe(0);
    });

    it('should handle errors gracefully', async () => {
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
      mockInvoke.mockRejectedValueOnce(new Error('Network error'));
      useApprovalStore.setState({ pendingApprovals: [mockRequest], currentApproval: mockRequest });

      await act(async () => {
        await useApprovalStore.getState().approve('req-1');
      });

      expect(consoleSpy).toHaveBeenCalledWith('Failed to approve action:', expect.any(Error));

      consoleSpy.mockRestore();
    });
  });

  describe('reject', () => {
    const mockRequest: ApprovalRequest = {
      id: 'req-1',
      tool_name: 'Edit',
      risk_level: 'mutating',
      summary: 'Edit file',
      input: { file_path: '/home/test.ts' },
      timestamp: Date.now(),
    };

    it('should reject a pending request', async () => {
      mockInvoke.mockResolvedValueOnce(undefined);
      useApprovalStore.setState({ pendingApprovals: [mockRequest], currentApproval: mockRequest });

      await act(async () => {
        await useApprovalStore.getState().reject('req-1');
      });

      expect(mockInvoke).toHaveBeenCalledWith('reject_action', { id: 'req-1', reason: undefined });

      const state = useApprovalStore.getState();
      expect(state.pendingApprovals).toEqual([]);
      expect(state.approvalHistory).toHaveLength(1);
      expect(state.approvalHistory[0].answer).toBe('no');
      expect(state.approvalHistory[0].autoApproved).toBe(false);
      expect(state.stats.rejected).toBe(1);
      expect(state.stats.total).toBe(1);
      expect(state.currentApproval).toBeNull();
    });

    it('should reject with reason', async () => {
      mockInvoke.mockResolvedValueOnce(undefined);
      useApprovalStore.setState({ pendingApprovals: [mockRequest], currentApproval: mockRequest });

      await act(async () => {
        await useApprovalStore.getState().reject('req-1', 'User cancelled');
      });

      expect(mockInvoke).toHaveBeenCalledWith('reject_action', { id: 'req-1', reason: 'User cancelled' });

      const state = useApprovalStore.getState();
      expect(state.approvalHistory[0].reason).toBe('User cancelled');
    });

    it('should do nothing if request not found', async () => {
      useApprovalStore.setState({ pendingApprovals: [mockRequest] });

      await act(async () => {
        await useApprovalStore.getState().reject('non-existent');
      });

      expect(mockInvoke).not.toHaveBeenCalled();
      expect(useApprovalStore.getState().stats.rejected).toBe(0);
    });

    it('should handle errors gracefully', async () => {
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
      mockInvoke.mockRejectedValueOnce(new Error('Network error'));
      useApprovalStore.setState({ pendingApprovals: [mockRequest], currentApproval: mockRequest });

      await act(async () => {
        await useApprovalStore.getState().reject('req-1');
      });

      expect(consoleSpy).toHaveBeenCalledWith('Failed to reject action:', expect.any(Error));

      consoleSpy.mockRestore();
    });
  });

  describe('abort', () => {
    const mockRequest: ApprovalRequest = {
      id: 'req-1',
      tool_name: 'Bash',
      risk_level: 'dangerous',
      summary: 'Delete files',
      input: { path: '/home' },
      timestamp: Date.now(),
    };

    it('should abort current approval', async () => {
      mockInvoke.mockResolvedValueOnce(undefined);
      useApprovalStore.setState({
        pendingApprovals: [mockRequest],
        currentApproval: mockRequest
      });

      await act(async () => {
        await useApprovalStore.getState().abort();
      });

      expect(mockInvoke).toHaveBeenCalledWith('reject_action', {
        id: 'req-1',
        reason: 'abort'
      });

      const state = useApprovalStore.getState();
      expect(state.pendingApprovals).toEqual([]);
      expect(state.approvalHistory).toHaveLength(1);
      expect(state.approvalHistory[0].answer).toBe('abort');
      expect(state.approvalHistory[0].reason).toBe('User aborted the turn');
      expect(state.stats.aborted).toBe(1);
      expect(state.stats.total).toBe(1);
      expect(state.currentApproval).toBeNull();
    });

    it('should do nothing if no current approval', async () => {
      useApprovalStore.setState({ currentApproval: null });

      await act(async () => {
        await useApprovalStore.getState().abort();
      });

      expect(mockInvoke).not.toHaveBeenCalled();
      expect(useApprovalStore.getState().stats.aborted).toBe(0);
    });

    it('should handle errors gracefully', async () => {
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
      mockInvoke.mockRejectedValueOnce(new Error('Network error'));
      useApprovalStore.setState({
        pendingApprovals: [mockRequest],
        currentApproval: mockRequest
      });

      await act(async () => {
        await useApprovalStore.getState().abort();
      });

      expect(consoleSpy).toHaveBeenCalledWith('Failed to abort:', expect.any(Error));

      consoleSpy.mockRestore();
    });
  });

  // ============================================================================
  // Event Listening Tests
  // ============================================================================

  describe('startListening', () => {
    it('should start listening for approval events', async () => {
      mockListen.mockResolvedValueOnce(mockUnlisten);

      await act(async () => {
        await useApprovalStore.getState().startListening();
      });

      expect(mockListen).toHaveBeenCalledWith('approval-request', expect.any(Function));
      expect(useApprovalStore.getState()._unlisten).toBe(mockUnlisten);
    });

    it('should not create duplicate listener', async () => {
      mockListen.mockResolvedValueOnce(mockUnlisten);

      await act(async () => {
        await useApprovalStore.getState().startListening();
        await useApprovalStore.getState().startListening();
      });

      expect(mockListen).toHaveBeenCalledTimes(1);
    });

    it('should add approval request when event is received', async () => {
      let eventCallback: (event: { payload: { approval: ApprovalRequest } }) => void;
      mockListen.mockImplementation((_event, callback) => {
        eventCallback = callback;
        return Promise.resolve(mockUnlisten);
      });

      await act(async () => {
        await useApprovalStore.getState().startListening();
      });

      const mockRequest: ApprovalRequest = {
        id: 'req-1',
        tool_name: 'Bash',
        risk_level: 'safe',
        summary: 'Test',
        input: {},
        timestamp: Date.now(),
      };

      act(() => {
        eventCallback!({ payload: { approval: mockRequest } });
      });

      const state = useApprovalStore.getState();
      expect(state.pendingApprovals).toHaveLength(1);
      expect(state.pendingApprovals[0]).toEqual(mockRequest);
      expect(state.currentApproval).toEqual(mockRequest);
    });

    it('should not overwrite current approval if already set', async () => {
      let eventCallback: (event: { payload: { approval: ApprovalRequest } }) => void;
      mockListen.mockImplementation((_event, callback) => {
        eventCallback = callback;
        return Promise.resolve(mockUnlisten);
      });

      const existingRequest: ApprovalRequest = {
        id: 'existing',
        tool_name: 'Read',
        risk_level: 'safe',
        summary: 'Existing',
        input: {},
        timestamp: Date.now(),
      };

      useApprovalStore.setState({ currentApproval: existingRequest });

      await act(async () => {
        await useApprovalStore.getState().startListening();
      });

      const mockRequest: ApprovalRequest = {
        id: 'req-1',
        tool_name: 'Bash',
        risk_level: 'safe',
        summary: 'Test',
        input: {},
        timestamp: Date.now(),
      };

      act(() => {
        eventCallback!({ payload: { approval: mockRequest } });
      });

      const state = useApprovalStore.getState();
      expect(state.pendingApprovals).toHaveLength(1);
      expect(state.currentApproval).toEqual(existingRequest); // Should remain unchanged
    });

    it('should handle errors gracefully', async () => {
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
      mockListen.mockRejectedValueOnce(new Error('Event error'));

      await act(async () => {
        await useApprovalStore.getState().startListening();
      });

      expect(consoleSpy).toHaveBeenCalledWith('Failed to start approval listener:', expect.any(Error));

      consoleSpy.mockRestore();
    });
  });

  describe('stopListening', () => {
    it('should stop listening and cleanup', async () => {
      mockListen.mockResolvedValueOnce(mockUnlisten);

      await act(async () => {
        await useApprovalStore.getState().startListening();
      });

      act(() => {
        useApprovalStore.getState().stopListening();
      });

      expect(mockUnlisten).toHaveBeenCalled();
      expect(useApprovalStore.getState()._unlisten).toBeNull();
    });

    it('should do nothing if not listening', () => {
      act(() => {
        useApprovalStore.getState().stopListening();
      });

      expect(mockUnlisten).not.toHaveBeenCalled();
    });
  });

  // ============================================================================
  // Manual Approval Request Management Tests
  // ============================================================================

  describe('addApprovalRequest', () => {
    it('should add request to pending queue', () => {
      const mockRequest: ApprovalRequest = {
        id: 'req-1',
        tool_name: 'Bash',
        risk_level: 'safe',
        summary: 'Test',
        input: {},
        timestamp: Date.now(),
      };

      act(() => {
        useApprovalStore.getState().addApprovalRequest(mockRequest);
      });

      const state = useApprovalStore.getState();
      expect(state.pendingApprovals).toHaveLength(1);
      expect(state.pendingApprovals[0]).toEqual(mockRequest);
      expect(state.currentApproval).toEqual(mockRequest);
    });

    it('should add multiple requests to queue', () => {
      const request1: ApprovalRequest = {
        id: 'req-1',
        tool_name: 'Bash',
        risk_level: 'safe',
        summary: 'Test 1',
        input: {},
        timestamp: Date.now(),
      };

      const request2: ApprovalRequest = {
        id: 'req-2',
        tool_name: 'Edit',
        risk_level: 'mutating',
        summary: 'Test 2',
        input: {},
        timestamp: Date.now(),
      };

      act(() => {
        useApprovalStore.getState().addApprovalRequest(request1);
        useApprovalStore.getState().addApprovalRequest(request2);
      });

      const state = useApprovalStore.getState();
      expect(state.pendingApprovals).toHaveLength(2);
      expect(state.currentApproval).toEqual(request1); // First request should be current
    });

    it('should not overwrite current approval when adding requests', () => {
      const request1: ApprovalRequest = {
        id: 'req-1',
        tool_name: 'Bash',
        risk_level: 'safe',
        summary: 'Test 1',
        input: {},
        timestamp: Date.now(),
      };

      const request2: ApprovalRequest = {
        id: 'req-2',
        tool_name: 'Edit',
        risk_level: 'mutating',
        summary: 'Test 2',
        input: {},
        timestamp: Date.now(),
      };

      act(() => {
        useApprovalStore.getState().addApprovalRequest(request1);
        useApprovalStore.getState().addApprovalRequest(request2);
      });

      const state = useApprovalStore.getState();
      expect(state.currentApproval).toEqual(request1);
      expect(state.pendingApprovals).toEqual([request1, request2]);
    });
  });

  describe('removeApprovalRequest', () => {
    it('should remove request from queue', () => {
      const request1: ApprovalRequest = {
        id: 'req-1',
        tool_name: 'Bash',
        risk_level: 'safe',
        summary: 'Test 1',
        input: {},
        timestamp: Date.now(),
      };

      const request2: ApprovalRequest = {
        id: 'req-2',
        tool_name: 'Edit',
        risk_level: 'mutating',
        summary: 'Test 2',
        input: {},
        timestamp: Date.now(),
      };

      useApprovalStore.setState({
        pendingApprovals: [request1, request2],
        currentApproval: request1
      });

      act(() => {
        useApprovalStore.getState().removeApprovalRequest('req-1');
      });

      const state = useApprovalStore.getState();
      expect(state.pendingApprovals).toHaveLength(1);
      expect(state.pendingApprovals[0]).toEqual(request2);
      expect(state.currentApproval).toEqual(request2);
    });

    it('should set currentApproval to null when queue becomes empty', () => {
      const request: ApprovalRequest = {
        id: 'req-1',
        tool_name: 'Bash',
        risk_level: 'safe',
        summary: 'Test',
        input: {},
        timestamp: Date.now(),
      };

      useApprovalStore.setState({
        pendingApprovals: [request],
        currentApproval: request
      });

      act(() => {
        useApprovalStore.getState().removeApprovalRequest('req-1');
      });

      const state = useApprovalStore.getState();
      expect(state.pendingApprovals).toEqual([]);
      expect(state.currentApproval).toBeNull();
    });

    it('should handle removing non-existent request', () => {
      const request: ApprovalRequest = {
        id: 'req-1',
        tool_name: 'Bash',
        risk_level: 'safe',
        summary: 'Test',
        input: {},
        timestamp: Date.now(),
      };

      useApprovalStore.setState({
        pendingApprovals: [request],
        currentApproval: request
      });

      act(() => {
        useApprovalStore.getState().removeApprovalRequest('non-existent');
      });

      const state = useApprovalStore.getState();
      expect(state.pendingApprovals).toHaveLength(1);
    });
  });

  describe('setCurrentApproval', () => {
    it('should set current approval', () => {
      const request: ApprovalRequest = {
        id: 'req-1',
        tool_name: 'Bash',
        risk_level: 'safe',
        summary: 'Test',
        input: {},
        timestamp: Date.now(),
      };

      act(() => {
        useApprovalStore.getState().setCurrentApproval(request);
      });

      expect(useApprovalStore.getState().currentApproval).toEqual(request);
    });

    it('should clear current approval when set to null', () => {
      const request: ApprovalRequest = {
        id: 'req-1',
        tool_name: 'Bash',
        risk_level: 'safe',
        summary: 'Test',
        input: {},
        timestamp: Date.now(),
      };

      useApprovalStore.setState({ currentApproval: request });

      act(() => {
        useApprovalStore.getState().setCurrentApproval(null);
      });

      expect(useApprovalStore.getState().currentApproval).toBeNull();
    });
  });

  // ============================================================================
  // Edge Cases and Integration Tests
  // ============================================================================

  describe('integration scenarios', () => {
    it('should handle complete approval workflow', async () => {
      mockInvoke.mockResolvedValueOnce(undefined);
      mockListen.mockResolvedValueOnce(mockUnlisten);

      const request: ApprovalRequest = {
        id: 'req-1',
        tool_name: 'Bash',
        risk_level: 'safe',
        summary: 'List files',
        input: { path: '/home' },
        timestamp: Date.now(),
      };

      // Start listening
      await act(async () => {
        await useApprovalStore.getState().startListening();
      });

      // Add request
      act(() => {
        useApprovalStore.getState().addApprovalRequest(request);
      });

      // Verify state
      let state = useApprovalStore.getState();
      expect(state.pendingApprovals).toHaveLength(1);
      expect(state.currentApproval).toEqual(request);

      // Approve request
      mockInvoke.mockResolvedValueOnce(undefined);
      await act(async () => {
        await useApprovalStore.getState().approve('req-1');
      });

      // Verify final state
      state = useApprovalStore.getState();
      expect(state.pendingApprovals).toEqual([]);
      expect(state.currentApproval).toBeNull();
      expect(state.stats.approved).toBe(1);

      // Stop listening
      act(() => {
        useApprovalStore.getState().stopListening();
      });

      expect(useApprovalStore.getState()._unlisten).toBeNull();
    });

    it('should handle multiple pending approvals', async () => {
      const requests: ApprovalRequest[] = [
        {
          id: 'req-1',
          tool_name: 'Bash',
          risk_level: 'safe',
          summary: 'Test 1',
          input: {},
          timestamp: Date.now(),
        },
        {
          id: 'req-2',
          tool_name: 'Edit',
          risk_level: 'mutating',
          summary: 'Test 2',
          input: {},
          timestamp: Date.now(),
        },
        {
          id: 'req-3',
          tool_name: 'Bash',
          risk_level: 'dangerous',
          summary: 'Test 3',
          input: {},
          timestamp: Date.now(),
        },
      ];

      // Add all requests
      act(() => {
        requests.forEach((req) => {
          useApprovalStore.getState().addApprovalRequest(req);
        });
      });

      expect(useApprovalStore.getState().pendingApprovals).toHaveLength(3);
      expect(useApprovalStore.getState().currentApproval).toEqual(requests[0]);

      // Approve first
      mockInvoke.mockResolvedValueOnce(undefined);
      await act(async () => {
        await useApprovalStore.getState().approve('req-1');
      });

      expect(useApprovalStore.getState().pendingApprovals).toHaveLength(2);
      expect(useApprovalStore.getState().currentApproval).toBeNull();

      // Set second as current
      act(() => {
        useApprovalStore.getState().setCurrentApproval(requests[1]);
      });

      // Reject second
      mockInvoke.mockResolvedValueOnce(undefined);
      await act(async () => {
        await useApprovalStore.getState().reject('req-2', 'User cancelled');
      });

      expect(useApprovalStore.getState().pendingApprovals).toHaveLength(1);
      expect(useApprovalStore.getState().stats.rejected).toBe(1);

      // Abort remaining
      act(() => {
        useApprovalStore.getState().setCurrentApproval(requests[2]);
      });

      mockInvoke.mockResolvedValueOnce(undefined);
      await act(async () => {
        await useApprovalStore.getState().abort();
      });

      const state = useApprovalStore.getState();
      expect(state.pendingApprovals).toEqual([]);
      expect(state.stats.approved).toBe(1);
      expect(state.stats.rejected).toBe(1);
      expect(state.stats.aborted).toBe(1);
      expect(state.stats.total).toBe(3);
    });
  });
});