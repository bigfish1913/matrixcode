import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { act } from '@testing-library/react';
import { ApproveModeDialog } from '../../src/components/ApproveModeDialog';
import type { ApprovalRequest, ApprovalRecord, ApprovalStats, ApprovalMode } from '../../src/stores/approvalStore';
import { mockInvoke, mockListen, mockUnlisten } from '../setup';

// Create a mock store state
const createMockStore = (stateOverrides = {}) => {
  const defaultState = {
    mode: 'ask' as ApprovalMode,
    pendingApprovals: [] as ApprovalRequest[],
    approvalHistory: [] as ApprovalRecord[],
    stats: { approved: 0, rejected: 0, autoApproved: 0, aborted: 0, total: 0 } as ApprovalStats,
    currentApproval: null as ApprovalRequest | null,
    loading: false,
    _unlisten: null,
    setMode: vi.fn().mockResolvedValue(undefined),
    loadMode: vi.fn().mockResolvedValue(undefined),
    loadHistory: vi.fn().mockResolvedValue(undefined),
    approve: vi.fn().mockResolvedValue(undefined),
    reject: vi.fn().mockResolvedValue(undefined),
    abort: vi.fn().mockResolvedValue(undefined),
    clearHistory: vi.fn(),
    startListening: vi.fn().mockResolvedValue(undefined),
    stopListening: vi.fn(),
    addApprovalRequest: vi.fn(),
    removeApprovalRequest: vi.fn(),
    setCurrentApproval: vi.fn(),
  };

  return { ...defaultState, ...stateOverrides };
};

// Mock zustand store
vi.mock('../../src/stores/approvalStore', () => ({
  useApprovalStore: vi.fn(),
  getRiskLevelIcon: (level: string) => {
    switch (level) {
      case 'safe': return 'information_source';
      case 'mutating': return 'pencil';
      case 'dangerous': return 'warning';
      default: return 'circle';
    }
  },
  getRiskLevelColor: (level: string) => {
    switch (level) {
      case 'safe': return 'text-blue-500';
      case 'mutating': return 'text-yellow-500';
      case 'dangerous': return 'text-red-500';
      default: return 'text-gray-500';
    }
  },
  getApprovalModeIcon: (mode: string) => {
    switch (mode) {
      case 'ask': return '❓';
      case 'auto': return '⚡';
      case 'strict': return '🔒';
      default: return '●';
    }
  },
  getApprovalModeColor: (mode: string) => {
    switch (mode) {
      case 'ask': return 'text-gray-500';
      case 'auto': return 'text-green-600';
      case 'strict': return 'text-red-600';
      default: return 'text-gray-500';
    }
  },
  getApprovalModeLabel: (mode: string) => {
    switch (mode) {
      case 'ask': return '询问';
      case 'auto': return '自动';
      case 'strict': return '严格';
      default: return '未知';
    }
  },
  getApprovalModeDescription: (mode: string) => {
    switch (mode) {
      case 'ask': return '每次操作都需要确认';
      case 'auto': return '自动执行安全操作';
      case 'strict': return '严格确认所有操作';
      default: return '';
    }
  },
}));

import { useApprovalStore } from '../../src/stores/approvalStore';

const mockedUseApprovalStore = vi.mocked(useApprovalStore);

describe('ApproveModeDialog', () => {
  const mockOnClose = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockReset();
    mockListen.mockReset();
    mockUnlisten.mockReset();
    mockedUseApprovalStore.mockReset();
  });

  describe('rendering', () => {
    it('should render the dialog with correct title', () => {
      const mockStore = createMockStore();
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      expect(screen.getByText('批准模式')).toBeInTheDocument();
      expect(screen.getByText('选择 Agent 执行操作的批准模式')).toBeInTheDocument();
    });

    it('should render all three mode options', () => {
      const mockStore = createMockStore();
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      // Use getAllByText since there are multiple elements with these texts
      expect(screen.getAllByText('询问').length).toBeGreaterThan(0);
      expect(screen.getAllByText('自动').length).toBeGreaterThan(0);
      expect(screen.getAllByText('严格').length).toBeGreaterThan(0);
    });

    it('should highlight current mode', () => {
      const mockStore = createMockStore({ mode: 'auto' });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      // Find the button with Auto text and check it has the primary border class
      const autoButtons = screen.getAllByText('自动');
      // The button should be the one in the mode selection panel
      const autoButton = autoButtons.find(el => el.closest('button')?.classList.contains('border-primary'));
      expect(autoButton).toBeTruthy();
    });

    it('should display current mode in header', () => {
      const mockStore = createMockStore({ mode: 'strict' });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      expect(screen.getByText(/当前模式:/)).toBeInTheDocument();
      // Use getAllByText and check that at least one Strict element exists
      expect(screen.getAllByText('严格').length).toBeGreaterThan(0);
    });

    it('should display statistics in header', () => {
      const mockStore = createMockStore({
        stats: { approved: 5, rejected: 2, autoApproved: 3, aborted: 1, total: 11 }
      });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      expect(screen.getByText(/批准 5/)).toBeInTheDocument();
      expect(screen.getByText(/拒绝 2/)).toBeInTheDocument();
      expect(screen.getByText(/自动 3/)).toBeInTheDocument();
    });

    it('should render close button', () => {
      const mockStore = createMockStore();
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      expect(screen.getByText('✕')).toBeInTheDocument();
    });
  });

  describe('mode selection', () => {
    it('should call setMode when mode is selected', async () => {
      const mockSetMode = vi.fn().mockResolvedValue(undefined);
      const mockStore = createMockStore({ setMode: mockSetMode });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      const autoButton = screen.getByText('自动').closest('button')!;
      await act(async () => {
        fireEvent.click(autoButton);
      });

      expect(mockSetMode).toHaveBeenCalledWith('auto');
    });

    it('should disable buttons while loading', async () => {
      const mockStore = createMockStore({ loading: true });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      const autoButton = screen.getByText('自动').closest('button')!;
      expect(autoButton).toBeDisabled();
    });

    it('should show selected mode with animation', async () => {
      const mockSetMode = vi.fn().mockResolvedValue(undefined);
      const mockStore = createMockStore({ setMode: mockSetMode });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      const autoButton = screen.getByText('自动').closest('button')!;
      await act(async () => {
        fireEvent.click(autoButton);
      });

      // Check for animate-pulse class
      expect(autoButton).toHaveClass('animate-pulse');
    });
  });

  describe('pending approvals', () => {
    const mockRequest: ApprovalRequest = {
      id: 'req-1',
      tool_name: 'Bash',
      risk_level: 'safe',
      summary: 'List files in directory',
      input: { path: '/home' },
      timestamp: Date.now(),
    };

    it('should display pending approvals section when there are pending requests', () => {
      const mockStore = createMockStore({
        pendingApprovals: [mockRequest],
        currentApproval: mockRequest,
      });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      expect(screen.getByText('待批准操作')).toBeInTheDocument();
      expect(screen.getByText('1 个')).toBeInTheDocument();
    });

    it('should not display pending approvals section when no pending requests', () => {
      const mockStore = createMockStore();
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      expect(screen.queryByText('待批准操作')).not.toBeInTheDocument();
    });

    it('should display current approval details', () => {
      const mockStore = createMockStore({
        pendingApprovals: [mockRequest],
        currentApproval: mockRequest,
      });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      expect(screen.getByText('List files in directory')).toBeInTheDocument();
      expect(screen.getByText(/工具: Bash/)).toBeInTheDocument();
      expect(screen.getByText(/风险: safe/)).toBeInTheDocument();
    });

    it('should display input JSON', () => {
      const mockStore = createMockStore({
        pendingApprovals: [mockRequest],
        currentApproval: mockRequest,
      });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      expect(screen.getByText(/"path": "\/home"/)).toBeInTheDocument();
    });

    it('should call approve when approve button is clicked', async () => {
      const mockApprove = vi.fn().mockResolvedValue(undefined);
      const mockStore = createMockStore({
        pendingApprovals: [mockRequest],
        currentApproval: mockRequest,
        approve: mockApprove,
      });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      const approveButton = screen.getByText('执行');
      await act(async () => {
        fireEvent.click(approveButton);
      });

      expect(mockApprove).toHaveBeenCalledWith('req-1');
    });

    it('should call reject when reject button is clicked', async () => {
      const mockReject = vi.fn().mockResolvedValue(undefined);
      const mockStore = createMockStore({
        pendingApprovals: [mockRequest],
        currentApproval: mockRequest,
        reject: mockReject,
      });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      const rejectButton = screen.getByText('跳过');
      await act(async () => {
        fireEvent.click(rejectButton);
      });

      expect(mockReject).toHaveBeenCalledWith('req-1', undefined);
    });

    it('should disable buttons while loading', () => {
      const mockStore = createMockStore({
        pendingApprovals: [mockRequest],
        currentApproval: mockRequest,
        loading: true,
      });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      expect(screen.getByText('执行')).toBeDisabled();
      expect(screen.getByText('跳过')).toBeDisabled();
    });

    it('should display queue items when there are multiple pending requests', () => {
      const request1: ApprovalRequest = {
        id: 'req-1',
        tool_name: 'Bash',
        risk_level: 'safe',
        summary: 'List files',
        input: {},
        timestamp: Date.now(),
      };

      const request2: ApprovalRequest = {
        id: 'req-2',
        tool_name: 'Edit',
        risk_level: 'mutating',
        summary: 'Edit file',
        input: {},
        timestamp: Date.now(),
      };

      const mockStore = createMockStore({
        pendingApprovals: [request1, request2],
        currentApproval: request1,
      });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      expect(screen.getByText('队列中:')).toBeInTheDocument();
      expect(screen.getByText('Edit file')).toBeInTheDocument();
    });

    it('should call setCurrentApproval when queue item is clicked', async () => {
      const mockSetCurrentApproval = vi.fn();
      const request1: ApprovalRequest = {
        id: 'req-1',
        tool_name: 'Bash',
        risk_level: 'safe',
        summary: 'List files',
        input: {},
        timestamp: Date.now(),
      };

      const request2: ApprovalRequest = {
        id: 'req-2',
        tool_name: 'Edit',
        risk_level: 'mutating',
        summary: 'Edit file',
        input: {},
        timestamp: Date.now(),
      };

      const mockStore = createMockStore({
        pendingApprovals: [request1, request2],
        currentApproval: request1,
        setCurrentApproval: mockSetCurrentApproval,
      });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      const queueItem = screen.getByText('Edit file').closest('div')!;
      await act(async () => {
        fireEvent.click(queueItem);
      });

      expect(mockSetCurrentApproval).toHaveBeenCalledWith(request2);
    });

    it('should call abort when abort button is clicked', async () => {
      const mockAbort = vi.fn().mockResolvedValue(undefined);
      const mockStore = createMockStore({
        pendingApprovals: [mockRequest],
        currentApproval: mockRequest,
        abort: mockAbort,
      });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      const abortButton = screen.getByText('中止本轮');
      await act(async () => {
        fireEvent.click(abortButton);
      });

      expect(mockAbort).toHaveBeenCalled();
      expect(mockOnClose).toHaveBeenCalled();
    });
  });

  describe('history panel', () => {
    it('should switch to history tab when clicked', () => {
      const mockStore = createMockStore();
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      const historyTab = screen.getByText(/历史记录/);
      fireEvent.click(historyTab);

      expect(screen.getByText('暂无批准历史记录')).toBeInTheDocument();
    });

    it('should display history records', () => {
      const mockHistory = [
        {
          id: 'record-1',
          request: {
            id: 'req-1',
            tool_name: 'Bash',
            risk_level: 'safe' as const,
            summary: 'List files',
            input: {},
            timestamp: Date.now(),
          },
          answer: 'yes' as const,
          timestamp: Date.now(),
          autoApproved: false,
        },
      ];

      const mockStore = createMockStore({
        approvalHistory: mockHistory,
        stats: { approved: 1, rejected: 0, autoApproved: 0, aborted: 0, total: 1 },
      });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      const historyTab = screen.getByText(/历史记录/);
      fireEvent.click(historyTab);

      expect(screen.getByText('List files')).toBeInTheDocument();
      expect(screen.getByText('执行')).toBeInTheDocument();
    });

    it('should display auto-approved badge', () => {
      const mockHistory = [
        {
          id: 'record-1',
          request: {
            id: 'req-1',
            tool_name: 'Bash',
            risk_level: 'safe' as const,
            summary: 'List files',
            input: {},
            timestamp: Date.now(),
          },
          answer: 'yes' as const,
          timestamp: Date.now(),
          autoApproved: true,
        },
      ];

      const mockStore = createMockStore({
        approvalHistory: mockHistory,
        stats: { approved: 0, rejected: 0, autoApproved: 1, aborted: 0, total: 1 },
      });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      const historyTab = screen.getByText(/历史记录/);
      fireEvent.click(historyTab);

      expect(screen.getByText('自动')).toBeInTheDocument();
    });

    it('should display rejected badge', () => {
      const mockHistory = [
        {
          id: 'record-1',
          request: {
            id: 'req-1',
            tool_name: 'Bash',
            risk_level: 'safe' as const,
            summary: 'List files',
            input: {},
            timestamp: Date.now(),
          },
          answer: 'no' as const,
          timestamp: Date.now(),
          autoApproved: false,
        },
      ];

      const mockStore = createMockStore({
        approvalHistory: mockHistory,
        stats: { approved: 0, rejected: 1, autoApproved: 0, aborted: 0, total: 1 },
      });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      const historyTab = screen.getByText(/历史记录/);
      fireEvent.click(historyTab);

      expect(screen.getByText('跳过')).toBeInTheDocument();
    });

    it('should display aborted badge', () => {
      const mockHistory = [
        {
          id: 'record-1',
          request: {
            id: 'req-1',
            tool_name: 'Bash',
            risk_level: 'safe' as const,
            summary: 'List files',
            input: {},
            timestamp: Date.now(),
          },
          answer: 'abort' as const,
          reason: 'User aborted the turn',
          timestamp: Date.now(),
          autoApproved: false,
        },
      ];

      const mockStore = createMockStore({
        approvalHistory: mockHistory,
        stats: { approved: 0, rejected: 0, autoApproved: 0, aborted: 1, total: 1 },
      });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      const historyTab = screen.getByText(/历史记录/);
      fireEvent.click(historyTab);

      // Use queryAllByText since there might be multiple "中止" texts (button and badge)
      const abortedBadges = screen.queryAllByText('中止');
      expect(abortedBadges.length).toBeGreaterThan(0);
    });

    it('should display reason when available', () => {
      const mockHistory = [
        {
          id: 'record-1',
          request: {
            id: 'req-1',
            tool_name: 'Bash',
            risk_level: 'safe' as const,
            summary: 'List files',
            input: {},
            timestamp: Date.now(),
          },
          answer: 'no' as const,
          reason: 'User cancelled',
          timestamp: Date.now(),
          autoApproved: false,
        },
      ];

      const mockStore = createMockStore({
        approvalHistory: mockHistory,
        stats: { approved: 0, rejected: 1, autoApproved: 0, aborted: 0, total: 1 },
      });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      const historyTab = screen.getByText(/历史记录/);
      fireEvent.click(historyTab);

      expect(screen.getByText(/原因: User cancelled/)).toBeInTheDocument();
    });

    it('should display statistics cards', () => {
      const mockStore = createMockStore({
        stats: { approved: 5, rejected: 2, autoApproved: 3, aborted: 1, total: 11 },
      });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      const historyTab = screen.getByText(/历史记录/);
      fireEvent.click(historyTab);

      expect(screen.getByText('5')).toBeInTheDocument();
      expect(screen.getByText('2')).toBeInTheDocument();
      expect(screen.getByText('3')).toBeInTheDocument();
      expect(screen.getByText('1')).toBeInTheDocument();
    });
  });

  describe('lifecycle', () => {
    it('should call loadHistory on mount', () => {
      const mockLoadHistory = vi.fn().mockResolvedValue(undefined);
      const mockStartListening = vi.fn().mockResolvedValue(undefined);
      const mockStore = createMockStore({
        loadHistory: mockLoadHistory,
        startListening: mockStartListening,
      });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      expect(mockLoadHistory).toHaveBeenCalled();
      expect(mockStartListening).toHaveBeenCalled();
    });

    it('should call stopListening on unmount', () => {
      const mockStopListening = vi.fn();
      const mockStore = createMockStore({ stopListening: mockStopListening });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      const { unmount } = render(<ApproveModeDialog onClose={mockOnClose} />);
      unmount();

      expect(mockStopListening).toHaveBeenCalled();
    });
  });

  describe('close functionality', () => {
    it('should call onClose when close button is clicked', () => {
      const mockStore = createMockStore();
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      const closeButton = screen.getByText('✕');
      fireEvent.click(closeButton);

      expect(mockOnClose).toHaveBeenCalled();
    });

    it('should call onClose when close button in footer is clicked', () => {
      const mockStore = createMockStore();
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      const footerCloseButton = screen.getByRole('button', { name: '关闭' });
      fireEvent.click(footerCloseButton);

      expect(mockOnClose).toHaveBeenCalled();
    });
  });

  describe('mode descriptions', () => {
    it('should display Ask mode features', () => {
      const mockStore = createMockStore();
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      expect(screen.getByText('工具执行前询问')).toBeInTheDocument();
      expect(screen.getByText('文件修改前确认')).toBeInTheDocument();
      expect(screen.getByText('推荐用于敏感操作')).toBeInTheDocument();
    });

    it('should display Auto mode features', () => {
      const mockStore = createMockStore({ mode: 'auto' });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      expect(screen.getByText('读取操作自动执行')).toBeInTheDocument();
      expect(screen.getByText('写入操作自动执行')).toBeInTheDocument();
      expect(screen.getByText('推荐用于日常开发')).toBeInTheDocument();
    });

    it('should display Strict mode features', () => {
      const mockStore = createMockStore({ mode: 'strict' });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      expect(screen.getByText('所有操作需确认')).toBeInTheDocument();
      expect(screen.getByText('包括读取文件')).toBeInTheDocument();
      expect(screen.getByText('推荐用于生产环境')).toBeInTheDocument();
    });
  });

  describe('edge cases', () => {
    it('should handle empty pending approvals queue gracefully', () => {
      const mockStore = createMockStore({ pendingApprovals: [], currentApproval: null });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      expect(screen.queryByText('待批准操作')).not.toBeInTheDocument();
    });

    it('should handle missing currentApproval when there are pending approvals', () => {
      const mockStore = createMockStore({
        pendingApprovals: [
          {
            id: 'req-1',
            tool_name: 'Bash',
            risk_level: 'safe' as const,
            summary: 'Test',
            input: {},
            timestamp: Date.now(),
          },
        ],
        currentApproval: null,
      });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      expect(screen.getByText('待批准操作')).toBeInTheDocument();
    });

    it('should handle long summary text', () => {
      const longSummary = 'This is a very long summary that should be displayed properly in the dialog without causing any layout issues or overflow problems';

      const mockStore = createMockStore({
        approvalHistory: [
          {
            id: 'record-1',
            request: {
              id: 'req-1',
              tool_name: 'Bash',
              risk_level: 'safe' as const,
              summary: longSummary,
              input: {},
              timestamp: Date.now(),
            },
            answer: 'yes' as const,
            timestamp: Date.now(),
            autoApproved: false,
          },
        ],
        stats: { approved: 1, rejected: 0, autoApproved: 0, aborted: 0, total: 1 },
      });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      const historyTab = screen.getByText(/历史记录/);
      fireEvent.click(historyTab);

      expect(screen.getByText(longSummary)).toBeInTheDocument();
    });

    it('should handle complex input JSON', () => {
      const complexInput = {
        path: '/home/user/projects/my-app/src/components/Button.tsx',
        options: { recursive: true, exclude: ['node_modules', '.git'] },
        flags: ['-a', '-l', '-h'],
      };

      const mockStore = createMockStore({
        pendingApprovals: [
          {
            id: 'req-1',
            tool_name: 'Bash',
            risk_level: 'safe' as const,
            summary: 'Complex operation',
            input: complexInput,
            timestamp: Date.now(),
          },
        ],
        currentApproval: {
          id: 'req-1',
          tool_name: 'Bash',
          risk_level: 'safe' as const,
          summary: 'Complex operation',
          input: complexInput,
          timestamp: Date.now(),
        },
      });
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      expect(screen.getByText(/recursive/)).toBeInTheDocument();
    });
  });

  describe('accessibility', () => {
    it('should have keyboard shortcut hint', () => {
      const mockStore = createMockStore();
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      expect(screen.getByText('Alt+M')).toBeInTheDocument();
      expect(screen.getByText('快捷切换')).toBeInTheDocument();
    });

    it('should have proper button roles', () => {
      const mockStore = createMockStore();
      mockedUseApprovalStore.mockImplementation((selector: any) => selector(mockStore));

      render(<ApproveModeDialog onClose={mockOnClose} />);

      const buttons = screen.getAllByRole('button');
      expect(buttons.length).toBeGreaterThan(0);
    });
  });
});