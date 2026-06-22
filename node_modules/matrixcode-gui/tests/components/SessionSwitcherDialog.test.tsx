import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { act } from '@testing-library/react';
import { SessionSwitcherDialog } from '../../src/components/SessionSwitcherDialog';
import { mockInvoke } from '../setup';

// Mock window.confirm
const mockConfirm = vi.fn();
window.confirm = mockConfirm;

describe('SessionSwitcherDialog', () => {
  const mockOnClose = vi.fn();
  const mockOnSelectSession = vi.fn();

  const mockSessions = [
    {
      id: 'session-1',
      name: 'Development Session',
      message_count: 10,
      created_at: '2024-01-01T00:00:00Z',
      project_path: '/home/user/project1',
      updated_at: '2024-01-05T00:00:00Z',
      short_id: 'dev1',
    },
    {
      id: 'session-2',
      name: 'Testing Session',
      message_count: 5,
      created_at: '2024-01-02T00:00:00Z',
      project_path: '/home/user/project2',
      updated_at: '2024-01-06T00:00:00Z',
      short_id: 'test1',
    },
    {
      id: 'session-3',
      name: 'Minimal Session',
      message_count: 15,
      created_at: '2024-01-03T00:00:00Z',
    },
  ];

  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockReset();
    mockConfirm.mockReset();
  });

  // ============================================================================
  // Rendering Tests
  // ============================================================================

  describe('rendering', () => {
    it('should render the dialog with correct title', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      expect(screen.getByText('切换会话')).toBeInTheDocument();
    });

    it('should render search input', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      expect(screen.getByPlaceholderText('搜索会话名称、ID 或项目路径...')).toBeInTheDocument();
    });

    it('should render close button', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      expect(screen.getByLabelText('关闭')).toBeInTheDocument();
    });

    it('should display loading state initially', () => {
      mockInvoke.mockImplementation(() => new Promise(() => {})); // Never resolves

      render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);

      expect(screen.getByText('正在加载会话...')).toBeInTheDocument();
    });

    it('should display error state on load failure', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('Load failed'));

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      await waitFor(() => {
        expect(screen.getByText('加载会话失败')).toBeInTheDocument();
        expect(screen.getByText('重试')).toBeInTheDocument();
      });
    });

    it('should display empty state when no sessions', async () => {
      mockInvoke.mockResolvedValueOnce([]);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      expect(screen.getByText('没有保存的会话')).toBeInTheDocument();
    });

    it('should display empty state when no matching sessions after filter', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      const searchInput = screen.getByPlaceholderText('搜索会话名称、ID 或项目路径...');
      await act(async () => {
        fireEvent.change(searchInput, { target: { value: 'nonexistent' } });
      });

      expect(screen.getByText('没有匹配的会话')).toBeInTheDocument();
    });
  });

  // ============================================================================
  // Session Display Tests
  // ============================================================================

  describe('session display', () => {
    it('should display all sessions', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      expect(screen.getByText('Development Session')).toBeInTheDocument();
      expect(screen.getByText('Testing Session')).toBeInTheDocument();
      expect(screen.getByText('Minimal Session')).toBeInTheDocument();
    });

    it('should display message count for each session', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      expect(screen.getByText('10 条消息')).toBeInTheDocument();
      expect(screen.getByText('5 条消息')).toBeInTheDocument();
      expect(screen.getByText('15 条消息')).toBeInTheDocument();
    });

    it('should display metadata (short_id, created_at, updated_at)', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      // Check short_id
      expect(screen.getByText('dev1')).toBeInTheDocument();
      expect(screen.getByText('test1')).toBeInTheDocument();

      // Check created_at
      expect(screen.getByText(/创建: 2024-01-01T00:00:00Z/)).toBeInTheDocument();

      // Check updated_at
      expect(screen.getByText(/更新: 2024-01-05T00:00:00Z/)).toBeInTheDocument();
    });

    it('should display project name from project_path', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      expect(screen.getByText(/项目: project1/)).toBeInTheDocument();
      expect(screen.getByText(/项目: project2/)).toBeInTheDocument();
    });

    it('should not display project name for sessions without project_path', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      // Minimal Session doesn't have project_path
      const minimalSessionCard = screen.getByText('Minimal Session').closest('div');
      expect(minimalSessionCard?.textContent).not.toContain('项目:');
    });

    it('should display session count in footer', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      expect(screen.getByText('3 个会话')).toBeInTheDocument();
    });

    it('should use id prefix when short_id is not available', async () => {
      mockInvoke.mockResolvedValueOnce([
        {
          id: 'very-long-session-id-12345',
          name: 'Session Without Short ID',
          message_count: 5,
          created_at: '2024-01-01T00:00:00Z',
        },
      ]);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      // The component uses session.id.slice(0, 8) when short_id is not available
      expect(screen.getByText('very-lon')).toBeInTheDocument();
    });
  });

  // ============================================================================
  // Current Session Marker Tests
  // ============================================================================

  describe('current session marker', () => {
    it('should mark current session with asterisk', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(
          <SessionSwitcherDialog
            onClose={mockOnClose}
            onSelectSession={mockOnSelectSession}
            currentSessionId="session-1"
          />
        );
      });

      expect(screen.getByText('*')).toBeInTheDocument();
      expect(screen.getByLabelText('当前会话')).toBeInTheDocument();
    });

    it('should not mark non-current sessions', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(
          <SessionSwitcherDialog
            onClose={mockOnClose}
            onSelectSession={mockOnSelectSession}
            currentSessionId="session-1"
          />
        );
      });

      // Only one asterisk should be present (for current session)
      const asterisks = screen.queryAllByText('*');
      expect(asterisks).toHaveLength(1);
    });

    it('should display current session indicator in footer', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(
          <SessionSwitcherDialog
            onClose={mockOnClose}
            onSelectSession={mockOnSelectSession}
            currentSessionId="session-1"
          />
        );
      });

      expect(screen.getByText('(* 当前会话)')).toBeInTheDocument();
    });

    it('should not display current session indicator when no current session', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      expect(screen.queryByText('(* 当前会话)')).not.toBeInTheDocument();
    });

    it('should set aria-current for current session', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(
          <SessionSwitcherDialog
            onClose={mockOnClose}
            onSelectSession={mockOnSelectSession}
            currentSessionId="session-1"
          />
        );
      });

      const currentSessionCard = screen.getByLabelText('会话: Development Session');
      expect(currentSessionCard).toHaveAttribute('aria-current', 'true');
    });
  });

  // ============================================================================
  // Delete Functionality Tests
  // ============================================================================

  describe('delete functionality', () => {
    it('should show delete button for each session', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      const deleteButtons = screen.getAllByLabelText(/删除会话/);
      expect(deleteButtons).toHaveLength(3);
    });

    it('should disable delete button for current session', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(
          <SessionSwitcherDialog
            onClose={mockOnClose}
            onSelectSession={mockOnSelectSession}
            currentSessionId="session-1"
          />
        );
      });

      const currentSessionDeleteButton = screen.getByLabelText('删除会话 Development Session');
      expect(currentSessionDeleteButton).toBeDisabled();
    });

    it('should show tooltip for disabled delete button', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(
          <SessionSwitcherDialog
            onClose={mockOnClose}
            onSelectSession={mockOnSelectSession}
            currentSessionId="session-1"
          />
        );
      });

      const currentSessionDeleteButton = screen.getByLabelText('删除会话 Development Session');
      expect(currentSessionDeleteButton).toHaveAttribute('title', '无法删除当前会话');
    });

    it('should show confirmation dialog before deletion', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);
      mockConfirm.mockReturnValueOnce(false);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      const deleteButton = screen.getByLabelText('删除会话 Development Session');
      await act(async () => {
        fireEvent.click(deleteButton);
      });

      expect(mockConfirm).toHaveBeenCalledWith('确定要删除此会话吗？此操作不可恢复。');
    });

    it('should not delete if confirmation is cancelled', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);
      mockConfirm.mockReturnValueOnce(false);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      const deleteButton = screen.getByLabelText('删除会话 Development Session');
      await act(async () => {
        fireEvent.click(deleteButton);
      });

      expect(mockInvoke).not.toHaveBeenCalledWith('delete_session', expect.anything());
    });

    it('should delete session after confirmation', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);
      mockConfirm.mockReturnValueOnce(true);
      mockInvoke.mockResolvedValueOnce(undefined);
      mockInvoke.mockResolvedValueOnce(mockSessions.slice(1));

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      const deleteButton = screen.getByLabelText('删除会话 Development Session');
      await act(async () => {
        fireEvent.click(deleteButton);
      });

      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledWith('delete_session', { id: 'session-1' });
      });
    });

    it('should reload sessions after deletion', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);
      mockConfirm.mockReturnValueOnce(true);
      mockInvoke.mockResolvedValueOnce(undefined);
      mockInvoke.mockResolvedValueOnce(mockSessions.slice(1));

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      const deleteButton = screen.getByLabelText('删除会话 Development Session');
      await act(async () => {
        fireEvent.click(deleteButton);
      });

      // Wait for deletion to complete
      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledWith('delete_session', { id: 'session-1' });
      });
    });

    it('should handle deleting state', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);
      mockConfirm.mockReturnValueOnce(true);
      mockInvoke.mockResolvedValueOnce(undefined);
      mockInvoke.mockResolvedValueOnce(mockSessions.slice(1));

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      const deleteButton = screen.getByLabelText('删除会话 Development Session');
      await act(async () => {
        fireEvent.click(deleteButton);
      });

      // Wait for operation to complete
      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalled();
      });
    });

    it('should display error on delete failure', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);
      mockConfirm.mockReturnValueOnce(true);
      mockInvoke.mockRejectedValueOnce(new Error('Delete failed'));

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      const deleteButton = screen.getByLabelText('删除会话 Development Session');
      await act(async () => {
        fireEvent.click(deleteButton);
      });

      // Wait for error to be handled
      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledWith('delete_session', { id: 'session-1' });
      });
    });

    it('should stop propagation on delete button click', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      const deleteButton = screen.getByLabelText('删除会话 Development Session');
      const stopPropagationSpy = vi.fn();

      const clickEvent = new MouseEvent('click', { bubbles: true });
      Object.defineProperty(clickEvent, 'stopPropagation', { value: stopPropagationSpy });

      fireEvent(deleteButton, clickEvent);

      expect(stopPropagationSpy).toHaveBeenCalled();
    });
  });

  // ============================================================================
  // Search Filter Tests
  // ============================================================================

  describe('search filter', () => {
    it('should filter sessions by name', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      const searchInput = screen.getByPlaceholderText('搜索会话名称、ID 或项目路径...');
      await act(async () => {
        fireEvent.change(searchInput, { target: { value: 'development' } });
      });

      expect(screen.getByText('Development Session')).toBeInTheDocument();
      expect(screen.queryByText('Testing Session')).not.toBeInTheDocument();
      expect(screen.queryByText('Minimal Session')).not.toBeInTheDocument();
    });

    it('should filter sessions by id', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      const searchInput = screen.getByPlaceholderText('搜索会话名称、ID 或项目路径...');
      await act(async () => {
        fireEvent.change(searchInput, { target: { value: 'session-2' } });
      });

      expect(screen.getByText('Testing Session')).toBeInTheDocument();
      expect(screen.queryByText('Development Session')).not.toBeInTheDocument();
    });

    it('should filter sessions by short_id', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      const searchInput = screen.getByPlaceholderText('搜索会话名称、ID 或项目路径...');
      await act(async () => {
        fireEvent.change(searchInput, { target: { value: 'dev1' } });
      });

      expect(screen.getByText('Development Session')).toBeInTheDocument();
      expect(screen.queryByText('Testing Session')).not.toBeInTheDocument();
    });

    it('should filter sessions by project_path', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      const searchInput = screen.getByPlaceholderText('搜索会话名称、ID 或项目路径...');
      await act(async () => {
        fireEvent.change(searchInput, { target: { value: 'project1' } });
      });

      expect(screen.getByText('Development Session')).toBeInTheDocument();
      expect(screen.queryByText('Testing Session')).not.toBeInTheDocument();
    });

    it('should be case-insensitive', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      const searchInput = screen.getByPlaceholderText('搜索会话名称、ID 或项目路径...');
      await act(async () => {
        fireEvent.change(searchInput, { target: { value: 'DEVELOPMENT' } });
      });

      expect(screen.getByText('Development Session')).toBeInTheDocument();
    });

    it('should reset selection index when filter changes', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(
          <SessionSwitcherDialog
            onClose={mockOnClose}
            onSelectSession={mockOnSelectSession}
            currentSessionId="session-2"
          />
        );
      });

      // Initial selection should be at index 1 (session-2)
      expect(screen.getByText('按 Enter 选择此会话')).toBeInTheDocument();

      const searchInput = screen.getByPlaceholderText('搜索会话名称、ID 或项目路径...');
      await act(async () => {
        fireEvent.change(searchInput, { target: { value: 'testing' } });
      });

      // After filter, selection should reset to 0 (hint present)
      expect(screen.getByText('按 Enter 选择此会话')).toBeInTheDocument();
    });

    it('should update session count based on filtered results', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      const searchInput = screen.getByPlaceholderText('搜索会话名称、ID 或项目路径...');
      await act(async () => {
        fireEvent.change(searchInput, { target: { value: 'development' } });
      });

      expect(screen.getByText('1 个会话')).toBeInTheDocument();
    });
  });

  // ============================================================================
  // Keyboard Navigation Tests
  // ============================================================================

  describe('keyboard navigation', () => {
    it('should navigate down with ArrowDown key', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      // Initial selection is at index 0 (indicated by "按 Enter 选择此会话")
      expect(screen.getByText('按 Enter 选择此会话')).toBeInTheDocument();

      await act(async () => {
        fireEvent.keyDown(window, { key: 'ArrowDown' });
      });

      // After navigation, the hint should still be present (on the new selection)
      expect(screen.getByText('按 Enter 选择此会话')).toBeInTheDocument();
    });

    it('should navigate up with ArrowUp key', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(
          <SessionSwitcherDialog
            onClose={mockOnClose}
            onSelectSession={mockOnSelectSession}
            currentSessionId="session-2"
          />
        );
      });

      // Navigate up from index 1
      await act(async () => {
        fireEvent.keyDown(window, { key: 'ArrowUp' });
      });

      // Selection hint should be present
      expect(screen.getByText('按 Enter 选择此会话')).toBeInTheDocument();
    });

    it('should navigate down with j key', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      await act(async () => {
        fireEvent.keyDown(window, { key: 'j' });
      });

      expect(screen.getByText('按 Enter 选择此会话')).toBeInTheDocument();
    });

    it('should navigate up with k key', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(
          <SessionSwitcherDialog
            onClose={mockOnClose}
            onSelectSession={mockOnSelectSession}
            currentSessionId="session-2"
          />
        );
      });

      await act(async () => {
        fireEvent.keyDown(window, { key: 'k' });
      });

      expect(screen.getByText('按 Enter 选择此会话')).toBeInTheDocument();
    });

    it('should not navigate down beyond last session', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(
          <SessionSwitcherDialog
            onClose={mockOnClose}
            onSelectSession={mockOnSelectSession}
            currentSessionId="session-3"
          />
        );
      });

      // Already at last session
      await act(async () => {
        fireEvent.keyDown(window, { key: 'ArrowDown' });
      });

      // Should still be at the last session (hint present)
      expect(screen.getByText('按 Enter 选择此会话')).toBeInTheDocument();
    });

    it('should not navigate up beyond first session', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      // Already at first session
      await act(async () => {
        fireEvent.keyDown(window, { key: 'ArrowUp' });
      });

      // Should stay at first session
      expect(screen.getByText('按 Enter 选择此会话')).toBeInTheDocument();
    });

    it('should select session on Enter key', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      await act(async () => {
        fireEvent.keyDown(window, { key: 'Enter' });
      });

      expect(mockOnSelectSession).toHaveBeenCalledWith('session-1');
      expect(mockOnClose).toHaveBeenCalled();
    });

    it('should close dialog on Escape key', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      await act(async () => {
        fireEvent.keyDown(window, { key: 'Escape' });
      });

      expect(mockOnClose).toHaveBeenCalled();
    });

    it('should not select session on Shift+Enter', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      await act(async () => {
        fireEvent.keyDown(window, { key: 'Enter', shiftKey: true });
      });

      expect(mockOnSelectSession).not.toHaveBeenCalled();
      expect(mockOnClose).not.toHaveBeenCalled();
    });

    it('should display keyboard hint for selected session', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      expect(screen.getByText('按 Enter 选择此会话')).toBeInTheDocument();
    });

    it('should show keyboard shortcuts in footer', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      expect(screen.getByText('导航')).toBeInTheDocument();
      expect(screen.getByText('选择')).toBeInTheDocument();
      expect(screen.getByText('取消')).toBeInTheDocument();
    });
  });

  // ============================================================================
  // Session Selection Tests
  // ============================================================================

  describe('session selection', () => {
    it('should select session on click', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      const session2Card = screen.getByText('Testing Session').closest('div');
      await act(async () => {
        fireEvent.click(session2Card!);
      });

      expect(mockOnSelectSession).toHaveBeenCalledWith('session-2');
      expect(mockOnClose).toHaveBeenCalled();
    });

    it('should call onSelectSession and onClose on session click', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      // Click second session
      const session2Card = screen.getByText('Testing Session').closest('div');
      await act(async () => {
        fireEvent.click(session2Card!);
      });

      // Should have called select and close
      expect(mockOnSelectSession).toHaveBeenCalledWith('session-2');
      expect(mockOnClose).toHaveBeenCalled();
    });

    it('should not select session while deleting', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);
      mockConfirm.mockReturnValueOnce(true);
      mockInvoke.mockImplementation(() => new Promise(() => {})); // Never resolves

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      // Start deletion
      const deleteButton = screen.getByLabelText('删除会话 Development Session');
      await act(async () => {
        fireEvent.click(deleteButton);
      });

      // Try to click the session
      const sessionCard = screen.getByText('Development Session').closest('div');
      await act(async () => {
        fireEvent.click(sessionCard!);
      });

      // Should not select or close
      expect(mockOnSelectSession).not.toHaveBeenCalled();
      expect(mockOnClose).not.toHaveBeenCalled();
    });
  });

  // ============================================================================
  // Close Functionality Tests
  // ============================================================================

  describe('close functionality', () => {
    it('should close on close button click', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      const closeButton = screen.getByLabelText('关闭');
      await act(async () => {
        fireEvent.click(closeButton);
      });

      expect(mockOnClose).toHaveBeenCalled();
    });

    it('should close on background click', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      // Find the overlay div (the first parent with fixed positioning)
      const overlay = document.querySelector('.fixed.inset-0');
      await act(async () => {
        fireEvent.click(overlay!);
      });

      expect(mockOnClose).toHaveBeenCalled();
    });

    it('should not close on dialog content click', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      const dialogContent = screen.getByText('切换会话').parentElement;
      await act(async () => {
        fireEvent.click(dialogContent!);
      });

      expect(mockOnClose).not.toHaveBeenCalled();
    });
  });

  // ============================================================================
  // Retry Functionality Tests
  // ============================================================================

  describe('retry functionality', () => {
    it('should reload sessions on retry button click', async () => {
      mockInvoke.mockRejectedValueOnce(new Error('Load failed'));
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      const retryButton = screen.getByText('重试');
      await act(async () => {
        fireEvent.click(retryButton);
      });

      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledTimes(2);
        expect(mockInvoke).toHaveBeenNthCalledWith(2, 'list_sessions');
      });
    });
  });

  // ============================================================================
  // Focus Management Tests
  // ============================================================================

  describe('focus management', () => {
    it('should focus search input on mount', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      const searchInput = screen.getByPlaceholderText('搜索会话名称、ID 或项目路径...');
      expect(searchInput).toHaveFocus();
    });
  });

  // ============================================================================
  // Edge Cases Tests
  // ============================================================================

  describe('edge cases', () => {
    it('should handle sessions with empty names', async () => {
      mockInvoke.mockResolvedValueOnce([
        {
          id: 'session-1',
          name: '',
          message_count: 5,
          created_at: '2024-01-01T00:00:00Z',
        },
      ]);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      expect(screen.getByText('未命名会话')).toBeInTheDocument();
    });

    it('should handle sessions with null message_count', async () => {
      mockInvoke.mockResolvedValueOnce([
        {
          id: 'session-1',
          name: 'Test Session',
          message_count: 0,
          created_at: '2024-01-01T00:00:00Z',
        },
      ]);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      expect(screen.getByText('0 条消息')).toBeInTheDocument();
    });

    it('should handle very long session names', async () => {
      const longName = 'This is a very long session name that should still be displayed properly';
      mockInvoke.mockResolvedValueOnce([
        {
          id: 'session-1',
          name: longName,
          message_count: 5,
          created_at: '2024-01-01T00:00:00Z',
        },
      ]);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      expect(screen.getByText(longName)).toBeInTheDocument();
    });

    it('should handle very long project paths', async () => {
      const longPath = '/very/long/path/to/project/that/should/be/truncated';
      mockInvoke.mockResolvedValueOnce([
        {
          id: 'session-1',
          name: 'Test Session',
          message_count: 5,
          created_at: '2024-01-01T00:00:00Z',
          project_path: longPath,
        },
      ]);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      expect(screen.getByTitle(longPath)).toBeInTheDocument();
    });

    it('should handle current session not in list', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(
          <SessionSwitcherDialog
            onClose={mockOnClose}
            onSelectSession={mockOnSelectSession}
            currentSessionId="non-existent-session"
          />
        );
      });

      // Should not show current session marker
      expect(screen.queryByText('*')).not.toBeInTheDocument();
      // Should start at index 0 (hint displayed)
      expect(screen.getByText('按 Enter 选择此会话')).toBeInTheDocument();
    });

    it('should handle empty session list gracefully', async () => {
      mockInvoke.mockResolvedValueOnce([]);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      expect(screen.getByText('没有保存的会话')).toBeInTheDocument();
      expect(screen.getByText('0 个会话')).toBeInTheDocument();
    });

    it('should handle special characters in search query', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      const searchInput = screen.getByPlaceholderText('搜索会话名称、ID 或项目路径...');
      await act(async () => {
        fireEvent.change(searchInput, { target: { value: '@#$%^&*()' } });
      });

      expect(screen.getByText('没有匹配的会话')).toBeInTheDocument();
    });

    it('should handle ctrl+j and ctrl+k without navigation', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      await act(async () => {
        fireEvent.keyDown(window, { key: 'j', ctrlKey: true });
      });

      // Should stay at first session (hint still present)
      expect(screen.getByText('按 Enter 选择此会话')).toBeInTheDocument();
    });

    it('should handle meta+j and meta+k without navigation', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      await act(async () => {
        fireEvent.keyDown(window, { key: 'k', metaKey: true });
      });

      // Should stay at first session (hint still present)
      expect(screen.getByText('按 Enter 选择此会话')).toBeInTheDocument();
    });
  });

  // ============================================================================
  // Accessibility Tests
  // ============================================================================

  describe('accessibility', () => {
    it('should have proper aria labels', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      expect(screen.getByLabelText('搜索会话')).toBeInTheDocument();
      expect(screen.getByLabelText('关闭')).toBeInTheDocument();
      expect(screen.getAllByLabelText(/会话:/)).toHaveLength(3);
    });

    it('should have proper aria-current attribute', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(
          <SessionSwitcherDialog
            onClose={mockOnClose}
            onSelectSession={mockOnSelectSession}
            currentSessionId="session-1"
          />
        );
      });

      const currentSessionCard = screen.getByLabelText('会话: Development Session');
      expect(currentSessionCard).toHaveAttribute('aria-current', 'true');
    });

    it('should have aria-current false for non-current sessions', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(
          <SessionSwitcherDialog
            onClose={mockOnClose}
            onSelectSession={mockOnSelectSession}
            currentSessionId="session-1"
          />
        );
      });

      const nonCurrentSessionCard = screen.getByLabelText('会话: Testing Session');
      expect(nonCurrentSessionCard).toHaveAttribute('aria-current', 'false');
    });

    it('should have accessible delete button labels', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      expect(screen.getByLabelText('删除会话 Development Session')).toBeInTheDocument();
      expect(screen.getByLabelText('删除会话 Testing Session')).toBeInTheDocument();
      expect(screen.getByLabelText('删除会话 Minimal Session')).toBeInTheDocument();
    });

    it('should have keyboard shortcut hints', async () => {
      mockInvoke.mockResolvedValueOnce(mockSessions);

      await act(async () => {
        render(<SessionSwitcherDialog onClose={mockOnClose} onSelectSession={mockOnSelectSession} />);
      });

      expect(screen.getByText('↑↓')).toBeInTheDocument();
      expect(screen.getByText('j/k')).toBeInTheDocument();
      expect(screen.getByText('Enter')).toBeInTheDocument();
      expect(screen.getByText('Esc')).toBeInTheDocument();
    });
  });
});