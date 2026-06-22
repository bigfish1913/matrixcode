import React, { useState, useEffect } from 'react';

// Language definitions
type Language = 'en' | 'zh' | 'ja' | 'ko' | 'es' | 'fr' | 'de' | 'ru';

// Translation strings
interface TranslationStrings {
  // General
  appName: string;
  loading: string;
  error: string;
  success: string;
  cancel: string;
  confirm: string;
  close: string;
  save: string;
  delete: string;
  edit: string;
  search: string;
  export: string;
  import: string;
  retry: string;
  clear: string;

  // Chat
  sendMessage: string;
  typeMessage: string;
  thinking: string;
  agentProcessing: string;
  newChat: string;
  continueSession: string;
  clearMessages: string;

  // Status
  statusIdle: string;
  statusRunning: string;
  statusError: string;

  // Messages
  userMessage: string;
  assistantMessage: string;
  toolMessage: string;
  errorMessage: string;
  thinkingContent: string;

  // Settings
  settings: string;
  apiProvider: string;
  apiKey: string;
  model: string;
  approveMode: string;
  theme: string;
  language: string;
  shortcuts: string;

  // Commands
  commandBar: string;
  searchMessages: string;
  showShortcuts: string;
  toggleDebug: string;
  toggleWorkflow: string;
  newSession: string;
  sessions: string;

  // Tokens
  tokens: string;
  inputTokens: string;
  outputTokens: string;
  cacheHit: string;

  // Export
  exportMarkdown: string;
  exportJson: string;
  exportHtml: string;
  exportTxt: string;
  exportPdf: string;
  exportCsv: string;

  // Errors
  networkError: string;
  apiError: string;
  timeoutError: string;
  unknownError: string;
}

// English translations
const ENGLISH: TranslationStrings = {
  appName: 'MatrixCode',
  loading: 'Loading...',
  error: 'Error',
  success: 'Success',
  cancel: 'Cancel',
  confirm: 'Confirm',
  close: 'Close',
  save: 'Save',
  delete: 'Delete',
  edit: 'Edit',
  search: 'Search',
  export: 'Export',
  import: 'Import',
  retry: 'Retry',
  clear: 'Clear',

  sendMessage: 'Send Message',
  typeMessage: 'Type a message...',
  thinking: 'Thinking...',
  agentProcessing: 'Agent is processing...',
  newChat: 'New Chat',
  continueSession: 'Continue',
  clearMessages: 'Clear Messages',

  statusIdle: 'Ready',
  statusRunning: 'Running',
  statusError: 'Error',

  userMessage: 'User',
  assistantMessage: 'Assistant',
  toolMessage: 'Tool',
  errorMessage: 'Error',
  thinkingContent: 'Thinking',

  settings: 'Settings',
  apiProvider: 'API Provider',
  apiKey: 'API Key',
  model: 'Model',
  approveMode: 'Approve Mode',
  theme: 'Theme',
  language: 'Language',
  shortcuts: 'Shortcuts',

  commandBar: 'Command Bar',
  searchMessages: 'Search Messages',
  showShortcuts: 'Show Shortcuts',
  toggleDebug: 'Toggle Debug',
  toggleWorkflow: 'Toggle Workflow',
  newSession: 'New Session',
  sessions: 'Sessions',

  tokens: 'Tokens',
  inputTokens: 'Input',
  outputTokens: 'Output',
  cacheHit: 'Cache',

  exportMarkdown: 'Markdown',
  exportJson: 'JSON',
  exportHtml: 'HTML',
  exportTxt: 'Plain Text',
  exportPdf: 'PDF',
  exportCsv: 'CSV',

  networkError: 'Network Error',
  apiError: 'API Error',
  timeoutError: 'Timeout',
  unknownError: 'Unknown Error',
};

// Chinese translations
const CHINESE: TranslationStrings = {
  appName: 'MatrixCode',
  loading: '加载中...',
  error: '错误',
  success: '成功',
  cancel: '取消',
  confirm: '确认',
  close: '关闭',
  save: '保存',
  delete: '删除',
  edit: '编辑',
  search: '搜索',
  export: '导出',
  import: '导入',
  retry: '重试',
  clear: '清空',

  sendMessage: '发送消息',
  typeMessage: '输入消息...',
  thinking: '思考中...',
  agentProcessing: 'Agent 处理中...',
  newChat: '新建会话',
  continueSession: '继续',
  clearMessages: '清空消息',

  statusIdle: '就绪',
  statusRunning: '运行中',
  statusError: '错误',

  userMessage: '用户',
  assistantMessage: '助手',
  toolMessage: '工具',
  errorMessage: '错误',
  thinkingContent: '思考',

  settings: '设置',
  apiProvider: 'API 提供商',
  apiKey: 'API 密钥',
  model: '模型',
  approveMode: '批准模式',
  theme: '主题',
  language: '语言',
  shortcuts: '快捷键',

  commandBar: '命令栏',
  searchMessages: '搜索消息',
  showShortcuts: '显示快捷键',
  toggleDebug: '切换调试',
  toggleWorkflow: '切换工作流',
  newSession: '新建会话',
  sessions: '会话',

  tokens: 'Token',
  inputTokens: '输入',
  outputTokens: '输出',
  cacheHit: '缓存',

  exportMarkdown: 'Markdown',
  exportJson: 'JSON',
  exportHtml: 'HTML',
  exportTxt: '纯文本',
  exportPdf: 'PDF',
  exportCsv: 'CSV',

  networkError: '网络错误',
  apiError: 'API 错误',
  timeoutError: '超时',
  unknownError: '未知错误',
};

// All translations
const TRANSLATIONS: Record<Language, TranslationStrings> = {
  en: ENGLISH,
  zh: CHINESE,
  ja: ENGLISH, // Fallback to English
  ko: ENGLISH,
  es: ENGLISH,
  fr: ENGLISH,
  de: ENGLISH,
  ru: ENGLISH,
};

// Language names
const LANGUAGE_NAMES: Record<Language, string> = {
  en: 'English',
  zh: '中文',
  ja: '日本語',
  ko: '한국어',
  es: 'Español',
  fr: 'Français',
  de: 'Deutsch',
  ru: 'Русский',
};

// Global language state
let currentLanguage: Language = 'zh';
const languageListeners: Set<(lang: Language) => void> = new Set();

// Load saved language
function loadLanguage(): Language {
  try {
    const saved = localStorage.getItem('matrixcode-language');
    if (saved && TRANSLATIONS[saved as Language]) {
      return saved as Language;
    }
  } catch {}

  // Detect browser language
  const browserLang = navigator.language.split('-')[0];
  if (TRANSLATIONS[browserLang as Language]) {
    return browserLang as Language;
  }

  return 'zh';
}

// Initialize
currentLanguage = loadLanguage();

// Change language
export function setLanguage(lang: Language): void {
  currentLanguage = lang;
  localStorage.setItem('matrixcode-language', lang);
  languageListeners.forEach(listener => listener(lang));
}

// Get current language
export function getLanguage(): Language {
  return currentLanguage;
}

// Get translation
export function t(key: keyof TranslationStrings): string {
  return TRANSLATIONS[currentLanguage][key] || TRANSLATIONS.en[key];
}

// Hook for language
export function useLanguage() {
  const [lang, setLang] = useState<Language>(currentLanguage);

  useEffect(() => {
    const listener = (newLang: Language) => setLang(newLang);
    languageListeners.add(listener);
    return () => {
      languageListeners.delete(listener);
    };
  }, []);

  return {
    language: lang,
    setLanguage: setLanguage,
    t,
    languages: Object.keys(LANGUAGE_NAMES) as Language[],
    languageNames: LANGUAGE_NAMES,
  };
}

// Language selector component
interface LanguageSelectorProps {
  onClose?: () => void;
}

export function LanguageSelector({ onClose }: LanguageSelectorProps) {
  const { language, setLanguage, languages, languageNames } = useLanguage();

  return (
    <div className="flex gap-2">
      {languages.map(lang => (
        <button
          key={lang}
          onClick={() => {
            setLanguage(lang);
            onClose?.();
          }}
          className={`px-3 py-1.5 rounded text-xs ${
            language === lang ? 'bg-primary text-primary-foreground' : 'bg-muted hover:bg-accent'
          }`}
        >
          {languageNames[lang]}
        </button>
      ))}
    </div>
  );
}