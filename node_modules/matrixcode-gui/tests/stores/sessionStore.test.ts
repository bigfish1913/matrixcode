import { describe, it, expect, beforeEach, vi } from 'vitest';
import { act } from '@testing-library/react';
import { useSessionStore, type SessionInfo } from '../../src/stores/sessionStore';
import { mockInvoke } from '../setup';

describe('sessionStore', () => {
  beforeEach(() => {
    // Reset store state before each test
    useSessionStore.setState({
      sessions: [],
      currentSessionId: null,
      loading: false,
      searchQuery: '',
    });
  });

  // ============================================================================
  // Initial State Tests
  // ============================================================================

  describe('initial state', () => {
    it('should have correct initial state', () => {
      const state = useSessionStore.getState();

      expect(state.sessions).toEqual([]);
      expect(state.currentSessionId).toBeNull();
      expect(state.loading).toBe(false);
      expect(state.searchQuery).toBe('');
    });
  });

  // ============================================================================
  // SessionInfo Metadata Validation Tests
  // ============================================================================

  describe('SessionInfo metadata validation', () => {
    it('should accept complete SessionInfo with all metadata fields', () => {
      const completeSession: SessionInfo = {
        id: 'session-123',
        name: 'Test Session',
        message_count: 10,
        created_at: '2024-01-01T00:00:00Z',
        project_path: '/home/user/project',
        updated_at: '2024-01-02T00:00:00Z',
        short_id: 'abc123',
      };

      expect(completeSession.id).toBeDefined();
      expect(completeSession.name).toBeDefined();
      expect(completeSession.message_count).toBeGreaterThanOrEqual(0);
      expect(completeSession.created_at).toBeDefined();
      expect(completeSession.project_path).toBeDefined();
      expect(completeSession.updated_at).toBeDefined();
      expect(completeSession.short_id).toBeDefined();
    });

    it('should accept SessionInfo without optional metadata fields', () => {
      const minimalSession: SessionInfo = {
        id: 'session-456',
        name: 'Minimal Session',
        message_count: 5,
        created_at: '2024-01-01T00:00:00Z',
      };

      expect(minimalSession.project_path).toBeUndefined();
      expect(minimalSession.updated_at).toBeUndefined();
      expect(minimalSession.short_id).toBeUndefined();
    });

    it('should handle SessionInfo with empty optional fields', () => {
      const sessionWithEmptyFields: SessionInfo = {
        id: 'session-789',
        name: 'Session with Empty Fields',
        message_count: 0,
        created_at: '2024-01-01T00:00:00Z',
        project_path: undefined,
        updated_at: undefined,
        short_id: undefined,
      };

      expect(sessionWithEmptyFields.project_path).toBeUndefined();
      expect(sessionWithEmptyFields.updated_at).toBeUndefined();
      expect(sessionWithEmptyFields.short_id).toBeUndefined();
    });
  });

  // ============================================================================
  // loadSessions Tests
  // ============================================================================

  describe('loadSessions', () => {
    it('should load sessions from backend', async () => {
      const mockSessions: SessionInfo[] = [
        {
          id: 'session-1',
          name: 'Session 1',
          message_count: 10,
          created_at: '2024-01-01T00:00:00Z',
          project_path: '/home/user/project1',
          updated_at: '2024-01-02T00:00:00Z',
          short_id: 'sess1',
        },
        {
          id: 'session-2',
          name: 'Session 2',
          message_count: 5,
          created_at: '2024-01-03T00:00:00Z',
          project_path: '/home/user/project2',
          updated_at: '2024-01-04T00:00:00Z',
          short_id: 'sess2',
        },
      ];

      mockInvoke.mockResolvedValueOnce(mockSessions);
      mockInvoke.mockResolvedValueOnce('session-1');

      await act(async () => {
        await useSessionStore.getState().loadSessions();
      });

      expect(mockInvoke).toHaveBeenCalledWith('list_sessions');
      expect(mockInvoke).toHaveBeenCalledWith('current_session');
      const state = useSessionStore.getState();
      expect(state.sessions).toEqual(mockSessions);
      expect(state.currentSessionId).toBe('session-1');
      expect(state.loading).toBe(false);
    });

    it('should handle empty session list', async () => {
      mockInvoke.mockResolvedValueOnce([]);
      mockInvoke.mockResolvedValueOnce(null);

      await act(async () => {
        await useSessionStore.getState().loadSessions();
      });

      const state = useSessionStore.getState();
      expect(state.sessions).toEqual([]);
      expect(state.currentSessionId).toBeNull();
      expect(state.loading).toBe(false);
    });

    it('should handle null current session', async () => {
      const mockSessions: SessionInfo[] = [
        {
          id: 'session-1',
          name: 'Session 1',
          message_count: 10,
          created_at: '2024-01-01T00:00:00Z',
        },
      ];

      mockInvoke.mockResolvedValueOnce(mockSessions);
      mockInvoke.mockResolvedValueOnce(null);

      await act(async () => {
        await useSessionStore.getState().loadSessions();
      });

      const state = useSessionStore.getState();
      expect(state.sessions).toEqual(mockSessions);
      expect(state.currentSessionId).toBeNull();
    });

    it('should handle loading state properly', async () => {
      mockInvoke.mockResolvedValueOnce([]);
      mockInvoke.mockResolvedValueOnce(null);

      // Start loading
      await act(async () => {
        await useSessionStore.getState().loadSessions();
      });

      // After loading completes, loading should be false
      expect(useSessionStore.getState().loading).toBe(false);
    });

    it('should handle errors gracefully', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('Network error'));

      await act(async () => {
        try {
          await useSessionStore.getState().loadSessions();
        } catch (e) {
          // Expected error
        }
      });

      expect(useSessionStore.getState().loading).toBe(false);
    });
  });

  // ============================================================================
  // createSession Tests
  // ============================================================================

  describe('createSession', () => {
    it('should create a new session with name', async () => {
      const newSessionId = 'new-session-123';
      mockInvoke.mockResolvedValueOnce(newSessionId);
      mockInvoke.mockResolvedValueOnce([]);
      mockInvoke.mockResolvedValueOnce(newSessionId);

      await act(async () => {
        const id = await useSessionStore.getState().createSession('New Session');
        expect(id).toBe(newSessionId);
      });

      expect(mockInvoke).toHaveBeenCalledWith('create_session', { name: 'New Session' });
      const state = useSessionStore.getState();
      expect(state.currentSessionId).toBe(newSessionId);
    });

    it('should create a new session without name', async () => {
      const newSessionId = 'new-session-456';
      mockInvoke.mockResolvedValueOnce(newSessionId);
      mockInvoke.mockResolvedValueOnce([]);
      mockInvoke.mockResolvedValueOnce(newSessionId);

      await act(async () => {
        const id = await useSessionStore.getState().createSession();
        expect(id).toBe(newSessionId);
      });

      expect(mockInvoke).toHaveBeenCalledWith('create_session', { name: null });
      const state = useSessionStore.getState();
      expect(state.currentSessionId).toBe(newSessionId);
    });

    it('should reload sessions after creating', async () => {
      const newSessionId = 'new-session-789';
      const mockSessions: SessionInfo[] = [
        {
          id: newSessionId,
          name: 'New Session',
          message_count: 0,
          created_at: '2024-01-01T00:00:00Z',
        },
      ];

      mockInvoke.mockResolvedValueOnce(newSessionId);
      mockInvoke.mockResolvedValueOnce(mockSessions);
      mockInvoke.mockResolvedValueOnce(newSessionId);

      await act(async () => {
        await useSessionStore.getState().createSession('New Session');
      });

      expect(mockInvoke).toHaveBeenCalledWith('list_sessions');
      const state = useSessionStore.getState();
      expect(state.sessions).toEqual(mockSessions);
    });
  });

  // ============================================================================
  // continueLast Tests
  // ============================================================================

  describe('continueLast', () => {
    it('should continue last session', async () => {
      const lastSessionId = 'last-session-123';
      mockInvoke.mockResolvedValueOnce(lastSessionId);
      mockInvoke.mockResolvedValueOnce([]);
      mockInvoke.mockResolvedValueOnce(lastSessionId);

      await act(async () => {
        const id = await useSessionStore.getState().continueLast();
        expect(id).toBe(lastSessionId);
      });

      expect(mockInvoke).toHaveBeenCalledWith('continue_last_session');
      const state = useSessionStore.getState();
      expect(state.currentSessionId).toBe(lastSessionId);
    });

    it('should return null if no last session', async () => {
      mockInvoke.mockResolvedValueOnce(null);

      await act(async () => {
        const id = await useSessionStore.getState().continueLast();
        expect(id).toBeNull();
      });

      const state = useSessionStore.getState();
      expect(state.currentSessionId).toBeNull();
    });

    it('should not reload sessions if no last session', async () => {
      mockInvoke.mockResolvedValueOnce(null);

      await act(async () => {
        await useSessionStore.getState().continueLast();
      });

      // Should not call list_sessions or current_session
      expect(mockInvoke).toHaveBeenCalledTimes(1);
      expect(mockInvoke).toHaveBeenCalledWith('continue_last_session');
    });
  });

  // ============================================================================
  // switchSession Tests
  // ============================================================================

  describe('switchSession', () => {
    it('should switch to specified session', async () => {
      const targetSessionId = 'target-session-123';
      mockInvoke.mockResolvedValueOnce(undefined);
      mockInvoke.mockResolvedValueOnce([]);
      mockInvoke.mockResolvedValueOnce(targetSessionId);

      await act(async () => {
        await useSessionStore.getState().switchSession(targetSessionId);
      });

      expect(mockInvoke).toHaveBeenCalledWith('switch_session', { sessionId: targetSessionId });
      const state = useSessionStore.getState();
      expect(state.currentSessionId).toBe(targetSessionId);
    });

    it('should reload sessions after switching', async () => {
      const targetSessionId = 'target-session-456';
      const mockSessions: SessionInfo[] = [
        {
          id: targetSessionId,
          name: 'Target Session',
          message_count: 15,
          created_at: '2024-01-01T00:00:00Z',
          updated_at: '2024-01-02T00:00:00Z',
        },
      ];

      mockInvoke.mockResolvedValueOnce(undefined);
      mockInvoke.mockResolvedValueOnce(mockSessions);
      mockInvoke.mockResolvedValueOnce(targetSessionId);

      await act(async () => {
        await useSessionStore.getState().switchSession(targetSessionId);
      });

      expect(mockInvoke).toHaveBeenCalledWith('list_sessions');
      const state = useSessionStore.getState();
      expect(state.sessions).toEqual(mockSessions);
    });

    it('should set currentSessionId before backend calls complete', async () => {
      const targetSessionId = 'target-session-789';
      mockInvoke.mockResolvedValueOnce(undefined);
      mockInvoke.mockResolvedValueOnce([]);
      mockInvoke.mockResolvedValueOnce(targetSessionId);

      await act(async () => {
        await useSessionStore.getState().switchSession(targetSessionId);
      });

      const state = useSessionStore.getState();
      expect(state.currentSessionId).toBe(targetSessionId);
    });
  });

  // ============================================================================
  // resumeSession Tests
  // ============================================================================

  describe('resumeSession', () => {
    it('should resume session with query', async () => {
      const resumedSessionId = 'resumed-session-123';
      const query = 'continue from here';
      mockInvoke.mockResolvedValueOnce(resumedSessionId);
      mockInvoke.mockResolvedValueOnce([]);
      mockInvoke.mockResolvedValueOnce(resumedSessionId);

      await act(async () => {
        const id = await useSessionStore.getState().resumeSession(query);
        expect(id).toBe(resumedSessionId);
      });

      expect(mockInvoke).toHaveBeenCalledWith('resume_session', { query });
      const state = useSessionStore.getState();
      expect(state.currentSessionId).toBe(resumedSessionId);
    });

    it('should return null if no session to resume', async () => {
      mockInvoke.mockResolvedValueOnce(null);

      await act(async () => {
        const id = await useSessionStore.getState().resumeSession('test query');
        expect(id).toBeNull();
      });

      const state = useSessionStore.getState();
      expect(state.currentSessionId).toBeNull();
    });

    it('should not reload sessions if no session to resume', async () => {
      mockInvoke.mockResolvedValueOnce(null);

      await act(async () => {
        await useSessionStore.getState().resumeSession('test query');
      });

      expect(mockInvoke).toHaveBeenCalledTimes(1);
      expect(mockInvoke).toHaveBeenCalledWith('resume_session', { query: 'test query' });
    });
  });

  // ============================================================================
  // renameSession Tests
  // ============================================================================

  describe('renameSession', () => {
    it('should rename current session', async () => {
      const newName = 'Renamed Session';
      mockInvoke.mockResolvedValueOnce(undefined);
      mockInvoke.mockResolvedValueOnce([]);
      mockInvoke.mockResolvedValueOnce('session-123');

      await act(async () => {
        await useSessionStore.getState().renameSession(newName);
      });

      expect(mockInvoke).toHaveBeenCalledWith('rename_session', { newName });
    });

    it('should reload sessions after renaming', async () => {
      const newName = 'Updated Name';
      const mockSessions: SessionInfo[] = [
        {
          id: 'session-123',
          name: newName,
          message_count: 10,
          created_at: '2024-01-01T00:00:00Z',
        },
      ];

      mockInvoke.mockResolvedValueOnce(undefined);
      mockInvoke.mockResolvedValueOnce(mockSessions);
      mockInvoke.mockResolvedValueOnce('session-123');

      await act(async () => {
        await useSessionStore.getState().renameSession(newName);
      });

      expect(mockInvoke).toHaveBeenCalledWith('list_sessions');
      const state = useSessionStore.getState();
      expect(state.sessions).toEqual(mockSessions);
    });
  });

  // ============================================================================
  // clearSession Tests
  // ============================================================================

  describe('clearSession', () => {
    it('should clear current session', async () => {
      mockInvoke.mockResolvedValueOnce(undefined);

      await act(async () => {
        await useSessionStore.getState().clearSession();
      });

      expect(mockInvoke).toHaveBeenCalledWith('clear_session');
    });
  });

  // ============================================================================
  // deleteSession Tests
  // ============================================================================

  describe('deleteSession', () => {
    it('should delete specified session', async () => {
      const sessionId = 'session-to-delete';
      mockInvoke.mockResolvedValueOnce(undefined);
      mockInvoke.mockResolvedValueOnce([]);
      mockInvoke.mockResolvedValueOnce('current-session');

      useSessionStore.setState({
        sessions: [
          { id: sessionId, name: 'ToDelete', message_count: 5, created_at: '2024-01-01T00:00:00Z' },
          { id: 'current-session', name: 'Current', message_count: 10, created_at: '2024-01-02T00:00:00Z' },
        ],
        currentSessionId: 'current-session',
      });

      await act(async () => {
        await useSessionStore.getState().deleteSession(sessionId);
      });

      expect(mockInvoke).toHaveBeenCalledWith('delete_session', { id: sessionId });
      expect(mockInvoke).toHaveBeenCalledWith('list_sessions');
    });

    it('should clear currentSessionId if deleting current session', async () => {
      const currentSessionId = 'current-session';
      mockInvoke.mockResolvedValueOnce(undefined);
      mockInvoke.mockResolvedValueOnce([]);
      mockInvoke.mockResolvedValueOnce(null);

      useSessionStore.setState({
        sessions: [
          { id: currentSessionId, name: 'Current', message_count: 10, created_at: '2024-01-01T00:00:00Z' },
        ],
        currentSessionId,
      });

      await act(async () => {
        await useSessionStore.getState().deleteSession(currentSessionId);
      });

      const state = useSessionStore.getState();
      expect(state.currentSessionId).toBeNull();
    });

    it('should not clear currentSessionId if deleting other session', async () => {
      const otherSessionId = 'other-session';
      const currentSessionId = 'current-session';
      mockInvoke.mockResolvedValueOnce(undefined);
      mockInvoke.mockResolvedValueOnce([
        { id: currentSessionId, name: 'Current', message_count: 10, created_at: '2024-01-01T00:00:00Z' },
      ]);
      mockInvoke.mockResolvedValueOnce(currentSessionId);

      useSessionStore.setState({
        sessions: [
          { id: otherSessionId, name: 'Other', message_count: 5, created_at: '2024-01-01T00:00:00Z' },
          { id: currentSessionId, name: 'Current', message_count: 10, created_at: '2024-01-02T00:00:00Z' },
        ],
        currentSessionId,
      });

      await act(async () => {
        await useSessionStore.getState().deleteSession(otherSessionId);
      });

      const state = useSessionStore.getState();
      expect(state.currentSessionId).toBe(currentSessionId);
    });

    it('should handle delete errors gracefully', async () => {
      const sessionId = 'session-to-delete';
      const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
      mockInvoke.mockRejectedValueOnce(new Error('Delete failed'));

      useSessionStore.setState({
        sessions: [
          { id: sessionId, name: 'ToDelete', message_count: 5, created_at: '2024-01-01T00:00:00Z' },
        ],
      });

      await act(async () => {
        try {
          await useSessionStore.getState().deleteSession(sessionId);
        } catch (e) {
          expect(e).toBeDefined();
        }
      });

      expect(consoleSpy).toHaveBeenCalledWith('Failed to delete session:', expect.any(Error));
      consoleSpy.mockRestore();
    });

    it('should throw error on delete failure', async () => {
      const sessionId = 'session-to-delete';
      mockInvoke.mockRejectedValueOnce(new Error('Delete failed'));

      useSessionStore.setState({
        sessions: [
          { id: sessionId, name: 'ToDelete', message_count: 5, created_at: '2024-01-01T00:00:00Z' },
        ],
      });

      let thrownError: Error | null = null;
      await act(async () => {
        try {
          await useSessionStore.getState().deleteSession(sessionId);
        } catch (e) {
          thrownError = e as Error;
        }
      });

      expect(thrownError).not.toBeNull();
      expect(thrownError?.message).toBe('Delete failed');
    });

    it('should reload sessions after successful delete', async () => {
      const sessionId = 'session-to-delete';
      const remainingSessions: SessionInfo[] = [
        {
          id: 'remaining-session',
          name: 'Remaining',
          message_count: 8,
          created_at: '2024-01-01T00:00:00Z',
          project_path: '/home/user/project',
          updated_at: '2024-01-02T00:00:00Z',
          short_id: 'remain',
        },
      ];

      mockInvoke.mockResolvedValueOnce(undefined);
      mockInvoke.mockResolvedValueOnce(remainingSessions);
      mockInvoke.mockResolvedValueOnce('remaining-session');

      useSessionStore.setState({
        sessions: [
          { id: sessionId, name: 'ToDelete', message_count: 5, created_at: '2024-01-01T00:00:00Z' },
          { id: 'remaining-session', name: 'Remaining', message_count: 8, created_at: '2024-01-02T00:00:00Z' },
        ],
        currentSessionId: 'remaining-session',
      });

      await act(async () => {
        await useSessionStore.getState().deleteSession(sessionId);
      });

      const state = useSessionStore.getState();
      expect(state.sessions).toEqual(remainingSessions);
    });
  });

  // ============================================================================
  // searchSessions Tests
  // ============================================================================

  describe('searchSessions', () => {
    it('should update searchQuery', () => {
      const query = 'test session';

      act(() => {
        useSessionStore.getState().searchSessions(query);
      });

      const state = useSessionStore.getState();
      expect(state.searchQuery).toBe(query);
    });

    it('should handle empty search query', () => {
      act(() => {
        useSessionStore.getState().searchSessions('');
      });

      const state = useSessionStore.getState();
      expect(state.searchQuery).toBe('');
    });

    it('should handle special characters in search query', () => {
      const query = 'session with special chars: @#$%^&*()';

      act(() => {
        useSessionStore.getState().searchSessions(query);
      });

      const state = useSessionStore.getState();
      expect(state.searchQuery).toBe(query);
    });

    it('should handle whitespace-only search query', () => {
      const query = '   ';

      act(() => {
        useSessionStore.getState().searchSessions(query);
      });

      const state = useSessionStore.getState();
      expect(state.searchQuery).toBe(query);
    });
  });

  // ============================================================================
  // setCurrentSession Tests
  // ============================================================================

  describe('setCurrentSession', () => {
    it('should set current session ID', () => {
      const sessionId = 'new-current-session';

      act(() => {
        useSessionStore.getState().setCurrentSession(sessionId);
      });

      const state = useSessionStore.getState();
      expect(state.currentSessionId).toBe(sessionId);
    });

    it('should clear current session ID when set to null', () => {
      useSessionStore.setState({ currentSessionId: 'existing-session' });

      act(() => {
        useSessionStore.getState().setCurrentSession(null);
      });

      const state = useSessionStore.getState();
      expect(state.currentSessionId).toBeNull();
    });
  });

  // ============================================================================
  // Integration Tests
  // ============================================================================

  describe('integration scenarios', () => {
    it('should handle complete session lifecycle', async () => {
      // Create session
      const sessionId = 'new-session';
      mockInvoke.mockResolvedValueOnce(sessionId);
      mockInvoke.mockResolvedValueOnce([
        { id: sessionId, name: 'New Session', message_count: 0, created_at: '2024-01-01T00:00:00Z' },
      ]);
      mockInvoke.mockResolvedValueOnce(sessionId);

      await act(async () => {
        await useSessionStore.getState().createSession('New Session');
      });

      expect(useSessionStore.getState().currentSessionId).toBe(sessionId);

      // Rename session
      mockInvoke.mockResolvedValueOnce(undefined);
      mockInvoke.mockResolvedValueOnce([
        { id: sessionId, name: 'Renamed Session', message_count: 0, created_at: '2024-01-01T00:00:00Z' },
      ]);
      mockInvoke.mockResolvedValueOnce(sessionId);

      await act(async () => {
        await useSessionStore.getState().renameSession('Renamed Session');
      });

      // Switch to another session
      const otherSessionId = 'other-session';
      mockInvoke.mockResolvedValueOnce(undefined);
      mockInvoke.mockResolvedValueOnce([
        { id: sessionId, name: 'Renamed Session', message_count: 0, created_at: '2024-01-01T00:00:00Z' },
        { id: otherSessionId, name: 'Other Session', message_count: 5, created_at: '2024-01-02T00:00:00Z' },
      ]);
      mockInvoke.mockResolvedValueOnce(otherSessionId);

      await act(async () => {
        await useSessionStore.getState().switchSession(otherSessionId);
      });

      expect(useSessionStore.getState().currentSessionId).toBe(otherSessionId);

      // Delete old session
      mockInvoke.mockResolvedValueOnce(undefined);
      mockInvoke.mockResolvedValueOnce([
        { id: otherSessionId, name: 'Other Session', message_count: 5, created_at: '2024-01-02T00:00:00Z' },
      ]);
      mockInvoke.mockResolvedValueOnce(otherSessionId);

      await act(async () => {
        await useSessionStore.getState().deleteSession(sessionId);
      });

      const state = useSessionStore.getState();
      expect(state.sessions).toHaveLength(1);
      expect(state.sessions[0].id).toBe(otherSessionId);
      expect(state.currentSessionId).toBe(otherSessionId);
    });

    it('should handle session search workflow', async () => {
      const mockSessions: SessionInfo[] = [
        { id: 'session-1', name: 'Development Session', message_count: 10, created_at: '2024-01-01T00:00:00Z' },
        { id: 'session-2', name: 'Testing Session', message_count: 5, created_at: '2024-01-02T00:00:00Z' },
        { id: 'session-3', name: 'Production Session', message_count: 15, created_at: '2024-01-03T00:00:00Z' },
      ];

      mockInvoke.mockResolvedValueOnce(mockSessions);
      mockInvoke.mockResolvedValueOnce('session-1');

      await act(async () => {
        await useSessionStore.getState().loadSessions();
      });

      // Search for sessions
      act(() => {
        useSessionStore.getState().searchSessions('test');
      });

      const state = useSessionStore.getState();
      expect(state.searchQuery).toBe('test');
      expect(state.sessions).toEqual(mockSessions);
    });

    it('should handle multiple sessions with metadata', async () => {
      const mockSessions: SessionInfo[] = [
        {
          id: 'session-1',
          name: 'Project A Session',
          message_count: 20,
          created_at: '2024-01-01T00:00:00Z',
          project_path: '/home/user/project-a',
          updated_at: '2024-01-05T00:00:00Z',
          short_id: 'proj-a',
        },
        {
          id: 'session-2',
          name: 'Project B Session',
          message_count: 15,
          created_at: '2024-01-02T00:00:00Z',
          project_path: '/home/user/project-b',
          updated_at: '2024-01-06T00:00:00Z',
          short_id: 'proj-b',
        },
        {
          id: 'session-3',
          name: 'Minimal Session',
          message_count: 5,
          created_at: '2024-01-03T00:00:00Z',
        },
      ];

      mockInvoke.mockResolvedValueOnce(mockSessions);
      mockInvoke.mockResolvedValueOnce('session-1');

      await act(async () => {
        await useSessionStore.getState().loadSessions();
      });

      const state = useSessionStore.getState();
      expect(state.sessions).toEqual(mockSessions);

      // Verify metadata is preserved
      expect(state.sessions[0].project_path).toBe('/home/user/project-a');
      expect(state.sessions[0].updated_at).toBe('2024-01-05T00:00:00Z');
      expect(state.sessions[0].short_id).toBe('proj-a');
      expect(state.sessions[2].project_path).toBeUndefined();
    });
  });
});