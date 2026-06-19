import React, { useState, useEffect } from 'react';

// Prompt template structure
interface PromptTemplate {
  id: string;
  name: string;
  category: 'development' | 'writing' | 'analysis' | 'debugging' | 'refactoring' | 'testing' | 'other';
  description: string;
  template: string;
  variables?: string[];  // Template variables like {{filename}}
  tags?: string[];
  favorite?: boolean;
}

// Built-in templates matching TUI slash commands
const BUILT_IN_TEMPLATES: PromptTemplate[] = [
  // Development templates
  {
    id: 'explain-code',
    name: 'Explain Code',
    category: 'development',
    description: '解释代码的工作原理',
    template: '请详细解释以下代码的工作原理:\n\n```{{language}}\n{{code}}\n```\n\n请包括:\n1. 整体逻辑流程\n2. 关键函数和变量\n3. 可能的改进建议',
    variables: ['language', 'code'],
    tags: ['code', 'explanation'],
  },
  {
    id: 'fix-bug',
    name: 'Fix Bug',
    category: 'debugging',
    description: '分析并修复代码错误',
    template: '请分析以下错误并修复:\n\n错误信息:\n```\n{{error}}\n```\n\n相关代码:\n```{{language}}\n{{code}}\n```\n\n请提供:\n1. 错误原因分析\n2. 修复方案\n3. 预防措施',
    variables: ['error', 'language', 'code'],
    tags: ['bug', 'fix'],
  },
  {
    id: 'write-test',
    name: 'Write Tests',
    category: 'testing',
    description: '为代码生成单元测试',
    template: '请为以下代码编写单元测试:\n\n```{{language}}\n{{code}}\n```\n\n测试要求:\n1. 覆盖主要功能\n2. 测试边界情况\n3. 使用标准测试框架',
    variables: ['language', 'code'],
    tags: ['test', 'unit-test'],
  },
  {
    id: 'refactor-code',
    name: 'Refactor Code',
    category: 'refactoring',
    description: '重构代码以提高质量',
    template: '请重构以下代码:\n\n```{{language}}\n{{code}}\n```\n\n重构目标:\n1. 提高可读性\n2. 减少重复代码\n3. 改善性能\n4. 保持功能不变',
    variables: ['language', 'code'],
    tags: ['refactor', 'clean-code'],
  },

  // Writing templates
  {
    id: 'write-docs',
    name: 'Write Documentation',
    category: 'writing',
    description: '为代码编写文档',
    template: '请为以下代码编写文档:\n\n```{{language}}\n{{code}}\n```\n\n文档应包括:\n1. 功能概述\n2. 参数说明\n3. 使用示例\n4. 注意事项',
    variables: ['language', 'code'],
    tags: ['docs', 'documentation'],
  },
  {
    id: 'write-comment',
    name: 'Add Comments',
    category: 'writing',
    description: '为代码添加注释',
    template: '请为以下代码添加详细注释:\n\n```{{language}}\n{{code}}\n```\n\n注释要求:\n1. 解释复杂逻辑\n2. 说明关键变量\n3. 保持简洁明了',
    variables: ['language', 'code'],
    tags: ['comment', 'annotate'],
  },

  // Analysis templates
  {
    id: 'analyze-performance',
    name: 'Analyze Performance',
    category: 'analysis',
    description: '分析代码性能问题',
    template: '请分析以下代码的性能:\n\n```{{language}}\n{{code}}\n```\n\n分析内容:\n1. 时间复杂度\n2. 空间复杂度\n3. 潜在瓶颈\n4. 优化建议',
    variables: ['language', 'code'],
    tags: ['performance', 'optimization'],
  },
  {
    id: 'security-review',
    name: 'Security Review',
    category: 'analysis',
    description: '检查代码安全问题',
    template: '请对以下代码进行安全审查:\n\n```{{language}}\n{{code}}\n```\n\n检查内容:\n1. 输入验证\n2. 潜在漏洞\n3. 数据安全\n4. 最佳安全实践',
    variables: ['language', 'code'],
    tags: ['security', 'review'],
  },

  // Other templates
  {
    id: 'generate-code',
    name: 'Generate Code',
    category: 'development',
    description: '根据需求生成代码',
    template: '请根据以下需求生成代码:\n\n需求:\n{{requirements}}\n\n语言: {{language}}\n\n要求:\n1. 实现所有需求\n2. 遵循最佳实践\n3. 包含必要注释',
    variables: ['requirements', 'language'],
    tags: ['generate', 'create'],
  },
  {
    id: 'convert-code',
    name: 'Convert Language',
    category: 'development',
    description: '将代码转换为其他语言',
    template: '请将以下代码从 {{from_language}} 转换为 {{to_language}}:\n\n```{{from_language}}\n{{code}}\n```\n\n保持:\n1. 功能一致\n2. 风格符合目标语言',
    variables: ['from_language', 'to_language', 'code'],
    tags: ['convert', 'translate'],
  },
];

// Template library store
interface TemplateLibraryState {
  templates: PromptTemplate[];
  favorites: string[];
  customTemplates: PromptTemplate[];
}

// Global template state
let templateLibrary: TemplateLibraryState = {
  templates: BUILT_IN_TEMPLATES,
  favorites: [],
  customTemplates: [],
};

const templateListeners: Set<(state: TemplateLibraryState) => void> = new Set();

// Add custom template
export function addCustomTemplate(template: PromptTemplate): void {
  templateLibrary.customTemplates.push(template);
  templateListeners.forEach(listener => listener(templateLibrary));

  // Save to localStorage
  saveTemplates();
}

// Remove custom template
export function removeCustomTemplate(id: string): void {
  templateLibrary.customTemplates = templateLibrary.customTemplates.filter(t => t.id !== id);
  templateListeners.forEach(listener => listener(templateLibrary));
  saveTemplates();
}

// Toggle favorite
export function toggleFavorite(id: string): void {
  const idx = templateLibrary.favorites.indexOf(id);
  if (idx >= 0) {
    templateLibrary.favorites.splice(idx, 1);
  } else {
    templateLibrary.favorites.push(id);
  }
  templateListeners.forEach(listener => listener(templateLibrary));
  saveTemplates();
}

// Get all templates
export function getAllTemplates(): PromptTemplate[] {
  return [...templateLibrary.templates, ...templateLibrary.customTemplates];
}

// Get templates by category
export function getTemplatesByCategory(category: PromptTemplate['category']): PromptTemplate[] {
  return getAllTemplates().filter(t => t.category === category);
}

// Get favorite templates
export function getFavoriteTemplates(): PromptTemplate[] {
  return getAllTemplates().filter(t => templateLibrary.favorites.includes(t.id));
}

// Load templates from localStorage
function loadTemplates(): void {
  try {
    const stored = localStorage.getItem('matrixcode-templates');
    if (stored) {
      const data = JSON.parse(stored);
      templateLibrary.customTemplates = data.customTemplates || [];
      templateLibrary.favorites = data.favorites || [];
    }
  } catch (e) {
    console.error('Failed to load templates:', e);
  }
}

// Save templates to localStorage
function saveTemplates(): void {
  try {
    localStorage.setItem('matrixcode-templates', JSON.stringify({
      customTemplates: templateLibrary.customTemplates,
      favorites: templateLibrary.favorites,
    }));
  } catch (e) {
    console.error('Failed to save templates:', e);
  }
}

// Initialize
loadTemplates();

// Template library dialog
interface TemplateLibraryDialogProps {
  onClose: () => void;
  onSelectTemplate: (template: PromptTemplate) => void;
}

export function TemplateLibraryDialog({ onClose, onSelectTemplate }: TemplateLibraryDialogProps) {
  const [state, setState] = useState<TemplateLibraryState>(templateLibrary);
  const [activeCategory, setActiveCategory] = useState<string>('all');
  const [searchQuery, setSearchQuery] = useState('');
  const [showFavorites, setShowFavorites] = useState(false);
  const [editingTemplate, setEditingTemplate] = useState<PromptTemplate | null>(null);
  const [previewTemplate, setPreviewTemplate] = useState<PromptTemplate | null>(null);

  // Subscribe to changes
  useEffect(() => {
    const listener = (newState: TemplateLibraryState) => {
      setState(newState);
    };
    templateListeners.add(listener);
    return () => {
      templateListeners.delete(listener);
    };
  }, []);

  // Categories
  const categories = ['all', 'favorites', 'development', 'debugging', 'writing', 'analysis', 'refactoring', 'testing', 'other'];

  // Category labels
  const categoryLabels: Record<string, string> = {
    all: '全部',
    favorites: '收藏',
    development: '开发',
    debugging: '调试',
    writing: '文档',
    analysis: '分析',
    refactoring: '重构',
    testing: '测试',
    other: '其他',
  };

  // Filter templates
  const filteredTemplates = getAllTemplates().filter(template => {
    // Category filter
    if (activeCategory === 'favorites') {
      if (!templateLibrary.favorites.includes(template.id)) return false;
    } else if (activeCategory !== 'all') {
      if (template.category !== activeCategory) return false;
    }

    // Search filter
    if (searchQuery) {
      const query = searchQuery.toLowerCase();
      return template.name.toLowerCase().includes(query) ||
             template.description.toLowerCase().includes(query) ||
             template.tags?.some(tag => tag.includes(query));
    }

    return true;
  });

  // Apply template (fill variables)
  const applyTemplate = (template: PromptTemplate) => {
    if (template.variables && template.variables.length > 0) {
      // Show preview with variables
      setPreviewTemplate(template);
    } else {
      // No variables, use directly
      onSelectTemplate(template);
      onClose();
    }
  };

  // Template card
  const TemplateCard = ({ template }: { template: PromptTemplate }) => (
    <div
      onClick={() => applyTemplate(template)}
      className="p-3 bg-muted/30 rounded-lg cursor-pointer hover:bg-accent/30 transition-colors"
    >
      <div className="flex items-start gap-3">
        {/* Favorite indicator */}
        <button
          onClick={(e) => {
            e.stopPropagation();
            toggleFavorite(template.id);
          }}
          className={`text-lg ${templateLibrary.favorites.includes(template.id) ? 'text-yellow-500' : 'text-muted-foreground'}`}
        >
          {templateLibrary.favorites.includes(template.id) ? '⭐' : '☆'}
        </button>

        {/* Content */}
        <div className="flex-1">
          <div className="font-medium text-sm">{template.name}</div>
          <div className="text-xs text-muted-foreground mt-1">{template.description}</div>

          {/* Tags */}
          {template.tags && (
            <div className="flex gap-1 mt-2">
              {template.tags.map(tag => (
                <span key={tag} className="px-1.5 py-0.5 bg-muted rounded text-xs">
                  {tag}
                </span>
              ))}
            </div>
          )}

          {/* Variables */}
          {template.variables && template.variables.length > 0 && (
            <div className="text-xs text-primary mt-1">
              Variables: {template.variables.join(', ')}
            </div>
          )}
        </div>
      </div>
    </div>
  );

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50 p-4">
      <div className="bg-card border shadow-lg rounded-lg max-w-2xl w-full max-h-[80vh] overflow-hidden flex">
        {/* Sidebar - Categories */}
        <div className="w-32 bg-muted/30 border-r">
          <div className="p-3 border-b">
            <h3 className="text-sm font-semibold">Templates</h3>
          </div>
          <div className="p-2">
            {categories.map(cat => (
              <button
                key={cat}
                onClick={() => setActiveCategory(cat)}
                className={`w-full p-2 rounded text-left text-xs ${
                  activeCategory === cat ? 'bg-primary/10 text-primary' : 'hover:bg-accent/30'
                }`}
              >
                {categoryLabels[cat]}
              </button>
            ))}
          </div>
        </div>

        {/* Main content */}
        <div className="flex-1 overflow-hidden flex flex-col">
          {/* Header */}
          <div className="p-4 border-b">
            <div className="flex items-center justify-between">
              <h3 className="font-semibold">Prompt Templates</h3>
              <button
                onClick={onClose}
                className="text-muted-foreground hover:text-foreground p-1 rounded hover:bg-accent"
              >
                ✕
              </button>
            </div>
          </div>

          {/* Search */}
          <div className="px-4 py-2 border-b">
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="搜索模板..."
              className="w-full bg-muted rounded px-3 py-1.5 text-sm outline-none focus:ring-2 focus:ring-primary"
            />
          </div>

          {/* Template list */}
          <div className="flex-1 overflow-y-auto p-4">
            <div className="space-y-2">
              {filteredTemplates.map(template => (
                <TemplateCard key={template.id} template={template} />
              ))}
            </div>
          </div>

          {/* Footer */}
          <div className="p-4 border-t bg-muted/30 text-xs text-muted-foreground">
            {filteredTemplates.length} templates |
            {templateLibrary.favorites.length} favorites
          </div>
        </div>
      </div>

      {/* Preview dialog */}
      {previewTemplate && (
        <TemplatePreviewDialog
          template={previewTemplate}
          onClose={() => setPreviewTemplate(null)}
          onApply={(filledTemplate) => {
            onSelectTemplate(filledTemplate);
            onClose();
          }}
        />
      )}
    </div>
  );
}

// Template preview dialog (for filling variables)
interface TemplatePreviewDialogProps {
  template: PromptTemplate;
  onClose: () => void;
  onApply: (template: PromptTemplate) => void;
}

function TemplatePreviewDialog({ template, onClose, onApply }: TemplatePreviewDialogProps) {
  const [variables, setVariables] = useState<Record<string, string>>({});

  // Fill template with variables
  const filledTemplate = template.template.replace(/\{\{(\w+)\}\}/g, (match, key) => {
    return variables[key] || match;
  });

  // Apply filled template
  const handleApply = () => {
    onApply({
      ...template,
      template: filledTemplate,
    });
  };

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-[60] p-4">
      <div className="bg-card border shadow-lg rounded-lg max-w-md w-full overflow-hidden">
        {/* Header */}
        <div className="p-4 border-b bg-muted/30">
          <h3 className="font-semibold">{template.name}</h3>
          <p className="text-sm text-muted-foreground">{template.description}</p>
        </div>

        {/* Variables input */}
        <div className="p-4">
          <div className="text-sm font-medium mb-2">Fill Variables:</div>
          {template.variables?.map(varName => (
            <div key={varName} className="mb-3">
              <label className="text-xs text-muted-foreground">{varName}</label>
              <input
                type="text"
                value={variables[varName] || ''}
                onChange={(e) => setVariables({ ...variables, [varName]: e.target.value })}
                placeholder={`Enter ${varName}`}
                className="w-full mt-1 px-3 py-2 bg-muted rounded text-sm outline-none focus:ring-2 focus:ring-primary"
              />
            </div>
          ))}

          {/* Preview */}
          <div className="mt-4">
            <div className="text-sm font-medium mb-2">Preview:</div>
            <div className="bg-muted/30 rounded p-3 text-sm">
              <pre className="whitespace-pre-wrap">{filledTemplate}</pre>
            </div>
          </div>
        </div>

        {/* Actions */}
        <div className="p-4 border-t bg-muted/30">
          <div className="flex gap-2">
            <button
              onClick={onClose}
              className="px-4 py-2 bg-muted text-muted-foreground rounded-lg text-sm hover:bg-accent"
            >
              Cancel
            </button>
            <button
              onClick={handleApply}
              className="flex-1 px-4 py-2 bg-primary text-primary-foreground rounded-lg text-sm hover:bg-primary/90"
            >
              Apply
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}