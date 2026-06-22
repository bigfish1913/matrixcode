import { create } from 'zustand';
import { useChatStore, type ChatMessage } from './chatStore';

// ============================================================================
// Types
// ============================================================================

/** Search result with match information */
export interface SearchResult {
  messageId: string;
  content: string;
  matchRange: { start: number; end: number };
  context: string;        // Surrounding text for preview
  role: 'user' | 'assistant' | 'tool' | 'error';
  timestamp: number;
  hasThinking: boolean;
  hasCode: boolean;
  toolName?: string;
}

/** Search filters */
export interface SearchFilters {
  role: 'all' | 'user' | 'assistant' | 'tool' | 'error';
  dateRange: 'all' | 'today' | 'week' | 'month';
  hasCode: boolean;
  hasThinking: boolean;
}

// ============================================================================
// Helper Functions
// ============================================================================

/** Get icon for role */
export function getRoleIcon(role: string): string {
  switch (role) {
    case 'user': return '👤';
    case 'assistant': return '🤖';
    case 'tool': return '🔧';
    case 'error': return '❌';
    default: return '●';
  }
}

/** Get color class for role */
export function getRoleColor(role: string): string {
  switch (role) {
    case 'user': return 'text-blue-500';
    case 'assistant': return 'text-green-500';
    case 'tool': return 'text-amber-500';
    case 'error': return 'text-red-500';
    default: return 'text-gray-500';
  }
}

/** Truncate content for preview */
export function truncateContent(content: string, maxLength: number = 150): string {
  if (content.length <= maxLength) return content;
  return content.slice(0, maxLength) + '...';
}

/** Find all matches in text */
export function findMatches(text: string, query: string): { start: number; end: number }[] {
  if (!query.trim()) return [];
  const matches: { start: number; end: number }[] = [];
  const queryLower = query.toLowerCase();
  let pos = 0;
  while (pos < text.length) {
    const idx = text.toLowerCase().indexOf(queryLower, pos);
    if (idx === -1) break;
    matches.push({ start: idx, end: idx + query.length });
    pos = idx + query.length;
  }
  return matches;
}

/** Get context around match */
export function getMatchContext(text: string, match: { start: number; end: number }, contextLength: number = 50): string {
  const contextStart = Math.max(0, match.start - contextLength);
  const contextEnd = Math.min(text.length, match.end + contextLength);
  let context = text.slice(contextStart, contextEnd);
  if (contextStart > 0) context = '...' + context;
  if (contextEnd < text.length) context = context + '...';
  return context;
}

// ============================================================================
// Store State Interface
// ============================================================================

interface SearchState {
  // Search query
  query: string;

  // Search results
  results: SearchResult[];

  // Search filters
  filters: SearchFilters;

  // Current result index for navigation
  currentResultIndex: number;

  // Search mode
  searchMode: 'content' | 'thinking' | 'tool';

  // Loading state
  loading: boolean;

  // Selected result for detail view
  selectedResult: SearchResult | null;

  // Actions
  search: (query: string, filters?: Partial<SearchFilters>) => void;
  searchInContent: (query: string) => void;
  searchInThinking: (query: string) => void;
  searchInTools: (query: string) => void;
  navigateNext: () => void;
  navigatePrev: () => void;
  setCurrentIndex: (index: number) => void;
  setFilters: (filters: Partial<SearchFilters>) => void;
  clearSearch: () => void;
  setSelectedResult: (result: SearchResult | null) => void;
}

// ============================================================================
// Default Filters
// ============================================================================

const DEFAULT_FILTERS: SearchFilters = {
  role: 'all',
  dateRange: 'all',
  hasCode: false,
  hasThinking: false,
};

// ============================================================================
// Store Implementation
// ============================================================================

export const useSearchStore = create<SearchState>((set, get) => ({
  query: '',
  results: [],
  filters: DEFAULT_FILTERS,
  currentResultIndex: 0,
  searchMode: 'content',
  loading: false,
  selectedResult: null,

  // Main search function
  search: (query: string, filters?: Partial<SearchFilters>) => {
    set({ loading: true, query });

    // Apply filters if provided
    if (filters) {
      set({ filters: { ...get().filters, ...filters } });
    }

    // Get messages from chat store
    const messages = useChatStore.getState().messages;
    const currentFilters = get().filters;

    // Filter messages
    let filtered = [...messages];

    // Role filter
    if (currentFilters.role !== 'all') {
      filtered = filtered.filter(m => m.role === currentFilters.role);
    }

    // Date range filter
    if (currentFilters.dateRange !== 'all') {
      const now = Date.now();
      const ranges: Record<string, number> = {
        today: 24 * 60 * 60 * 1000,
        week: 7 * 24 * 60 * 60 * 1000,
        month: 30 * 24 * 60 * 60 * 1000,
      };
      const rangeMs = ranges[currentFilters.dateRange];
      // Validate rangeMs - if invalid, skip date range filter
      if (rangeMs !== undefined) {
        filtered = filtered.filter(m => {
          if (!m.timestamp) return false;
          return now - m.timestamp < rangeMs;
        });
      }
    }

    // Has code filter - improved detection for actual code blocks
    if (currentFilters.hasCode) {
      filtered = filtered.filter(m => {
        // Check for code blocks (triple backticks)
        if (m.content.includes('```')) return true;
        // Check for inline code with balanced backticks
        const backtickCount = (m.content.match(/`[^`]+`/g) || []).length;
        if (backtickCount > 0) return true;
        return false;
      });
    }

    // Has thinking filter
    if (currentFilters.hasThinking) {
      filtered = filtered.filter(m => m.thinking && m.thinking.length > 0);
    }

    // Search query - respect searchMode for field-specific search
    if (query.trim() !== '') {
      const queryLower = query.toLowerCase();
      const searchMode = get().searchMode;

      filtered = filtered.filter(m => {
        switch (searchMode) {
          case 'content':
            // Only search in content field
            return m.content.toLowerCase().includes(queryLower);
          case 'thinking':
            // Only search in thinking field (requires hasThinking filter)
            return m.thinking?.toLowerCase().includes(queryLower) ?? false;
          case 'tool':
            // Only search in toolName field
            return m.toolName?.toLowerCase().includes(queryLower) ?? false;
          default:
            // Default: search all fields
            if (m.content.toLowerCase().includes(queryLower)) return true;
            if (m.thinking?.toLowerCase().includes(queryLower)) return true;
            if (m.toolName?.toLowerCase().includes(queryLower)) return true;
            return false;
        }
      });
    }

    // Build search results
    const results: SearchResult[] = filtered.map(m => {
      // Find first match in content or thinking
      let matchRange = { start: 0, end: 0 };
      let context = '';

      if (query.trim() !== '') {
        // Search in content first
        const contentMatches = findMatches(m.content, query);
        if (contentMatches.length > 0) {
          matchRange = contentMatches[0];
          context = getMatchContext(m.content, matchRange);
        } else if (m.thinking) {
          // Search in thinking if no content match
          const thinkingMatches = findMatches(m.thinking, query);
          if (thinkingMatches.length > 0) {
            matchRange = thinkingMatches[0];
            context = getMatchContext(m.thinking, matchRange);
          }
        }
      } else {
        // No query - show full content
        context = truncateContent(m.content);
      }

      return {
        messageId: m.id,
        content: m.content,
        matchRange,
        context,
        role: m.role,
        timestamp: m.timestamp || Date.now(),
        hasThinking: Boolean(m.thinking),
        hasCode: m.content.includes('```'),
        toolName: m.toolName,
      };
    });

    set({
      results,
      currentResultIndex: 0,
      loading: false,
      selectedResult: results.length > 0 ? results[0] : null,
    });
  },

  // Search only in content
  searchInContent: (query: string) => {
    set({ searchMode: 'content' });
    get().search(query);
  },

  // Search only in thinking
  searchInThinking: (query: string) => {
    set({ searchMode: 'thinking', filters: { ...get().filters, hasThinking: true } });
    get().search(query);
  },

  // Search only in tools
  searchInTools: (query: string) => {
    set({ searchMode: 'tool', filters: { ...get().filters, role: 'tool' } });
    get().search(query);
  },

  // Navigate to next result
  navigateNext: () => {
    const { results, currentResultIndex } = get();
    if (results.length === 0) return;
    const nextIndex = (currentResultIndex + 1) % results.length;
    set({
      currentResultIndex: nextIndex,
      selectedResult: results[nextIndex],
    });
  },

  // Navigate to previous result
  navigatePrev: () => {
    const { results, currentResultIndex } = get();
    if (results.length === 0) return;
    const prevIndex = currentResultIndex === 0 ? results.length - 1 : currentResultIndex - 1;
    set({
      currentResultIndex: prevIndex,
      selectedResult: results[prevIndex],
    });
  },

  // Set current result index
  setCurrentIndex: (index: number) => {
    const { results } = get();
    if (index >= 0 && index < results.length) {
      set({
        currentResultIndex: index,
        selectedResult: results[index],
      });
    }
  },

  // Set filters
  setFilters: (filters: Partial<SearchFilters>) => {
    set({ filters: { ...get().filters, ...filters } });
    // Re-run search with new filters
    if (get().query.trim() !== '') {
      get().search(get().query);
    }
  },

  // Clear search
  clearSearch: () => {
    set({
      query: '',
      results: [],
      currentResultIndex: 0,
      filters: DEFAULT_FILTERS,
      searchMode: 'content',
      selectedResult: null,
    });
  },

  // Set selected result
  setSelectedResult: (result: SearchResult | null) => {
    set({ selectedResult: result });
  },
}));

// ============================================================================
// Highlight Helper (used in components)
// ============================================================================

/** Highlight search query in text - returns array of parts for rendering */
export function highlightQueryParts(text: string, query: string): Array<{ type: 'text' | 'match'; content: string }> {
  if (!query.trim()) return [{ type: 'text', content: text }];

  const matches = findMatches(text, query);
  const parts: Array<{ type: 'text' | 'match'; content: string }> = [];
  let lastEnd = 0;

  matches.forEach((match) => {
    // Add text before match
    if (match.start > lastEnd) {
      parts.push({ type: 'text', content: text.slice(lastEnd, match.start) });
    }
    // Add highlighted match
    parts.push({ type: 'match', content: text.slice(match.start, match.end) });
    lastEnd = match.end;
  });

  // Add remaining text
  if (lastEnd < text.length) {
    parts.push({ type: 'text', content: text.slice(lastEnd) });
  }

  return parts;
}

// ============================================================================
// Default Export
// ============================================================================

export default useSearchStore;