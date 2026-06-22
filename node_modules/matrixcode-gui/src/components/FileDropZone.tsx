import React, { useState, useRef, useCallback } from 'react';

interface FileDropZoneProps {
  onFileDrop: (files: FileList) => void;
  onPathDrop?: (path: string) => void;
  children: React.ReactNode;
  disabled?: boolean;
}

// Supported file types for drop
const SUPPORTED_FILE_TYPES = [
  // Code files
  '.js', '.jsx', '.ts', '.tsx', '.py', '.rs', '.go', '.java', '.cpp', '.c', '.rb', '.php',
  '.swift', '.kt', '.scala', '.vue', '.svelte', '.astro',
  // Config files
  '.json', '.yaml', '.yml', '.toml', '.ini', '.env',
  // Text files
  '.md', '.txt', '.csv', '.log',
  // Web files
  '.html', '.css', '.scss', '.sass', '.less',
  // Shell files
  '.sh', '.bash', '.zsh', '.ps1',
  // Other
  '.sql', '.graphql', '.xml',
];

// Check if file is supported
function isFileSupported(file: File): boolean {
  const ext = '.' + file.name.split('.').pop()?.toLowerCase();
  return SUPPORTED_FILE_TYPES.includes(ext) || file.type.startsWith('text/');
}

// Format file size
function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(2)} MB`;
}

export function FileDropZone({
  onFileDrop,
  onPathDrop,
  children,
  disabled = false,
}: FileDropZoneProps) {
  const [isDragging, setIsDragging] = useState(false);
  const [draggedFiles, setDraggedFiles] = useState<File[]>([]);
  const [error, setError] = useState<string | null>(null);
  const dropRef = useRef<HTMLDivElement>(null);

  // Handle drag enter
  const handleDragEnter = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (disabled) return;

    setIsDragging(true);

    // Get dragged files
    const files = Array.from(e.dataTransfer.files);
    setDraggedFiles(files);
    setError(null);
  }, [disabled]);

  // Handle drag over
  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (disabled) return;
  }, [disabled]);

  // Handle drag leave
  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();

    // Only set dragging false if leaving the drop zone
    if (dropRef.current && !dropRef.current.contains(e.relatedTarget as Node)) {
      setIsDragging(false);
      setDraggedFiles([]);
    }
  }, []);

  // Handle drop
  const handleDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragging(false);
    setDraggedFiles([]);

    if (disabled) return;

    const files = e.dataTransfer.files;
    if (files.length === 0) return;

    // Check if all files are supported
    const unsupportedFiles = Array.from(files).filter(f => !isFileSupported(f));
    if (unsupportedFiles.length > 0) {
      setError(`不支持的文件类型: ${unsupportedFiles.map(f => f.name).join(', ')}`);
      return;
    }

    // Check file size limit (10MB per file)
    const oversizedFiles = Array.from(files).filter(f => f.size > 10 * 1024 * 1024);
    if (oversizedFiles.length > 0) {
      setError(`文件太大: ${oversizedFiles.map(f => `${f.name} (${formatFileSize(f.size)})`).join(', ')}`);
      return;
    }

    setError(null);
    onFileDrop(files);
  }, [disabled, onFileDrop]);

  // Handle paste (Ctrl+V) for files
  const handlePaste = useCallback(async (e: React.ClipboardEvent) => {
    if (disabled) return;

    const items = e.clipboardData.items;
    const fileItems = Array.from(items).filter(item => item.kind === 'file');

    if (fileItems.length > 0) {
      e.preventDefault();
      const files = await Promise.all(
        fileItems.map(item => item.getAsFile())
      );
      const validFiles = files.filter(f => f !== null) as File[];

      if (validFiles.length > 0) {
        onFileDrop(validFiles as unknown as FileList);
      }
    }
  }, [disabled, onFileDrop]);

  return (
    <div
      ref={dropRef}
      onDragEnter={handleDragEnter}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
      onPaste={handlePaste}
      className={`relative ${isDragging ? 'ring-2 ring-primary ring-offset-2' : ''}`}
    >
      {/* Drop overlay */}
      {isDragging && (
        <div className="absolute inset-0 bg-primary/10 backdrop-blur-sm z-20 flex items-center justify-center">
          <div className="bg-card border shadow-lg rounded-lg p-4 max-w-sm w-full">
            <div className="text-center">
              <div className="text-4xl mb-2">📁</div>
              <h3 className="font-semibold mb-2">拖放文件</h3>

              {/* Preview dropped files */}
              {draggedFiles.length > 0 && (
                <div className="mt-3 space-y-1">
                  {draggedFiles.slice(0, 5).map((file, idx) => (
                    <div key={idx} className="flex items-center gap-2 text-xs">
                      <span className={isFileSupported(file) ? 'text-green-500' : 'text-red-500'}>
                        {isFileSupported(file) ? '✓' : '✗'}
                      </span>
                      <span className="truncate">{file.name}</span>
                      <span className="text-muted-foreground">{formatFileSize(file.size)}</span>
                    </div>
                  ))}
                  {draggedFiles.length > 5 && (
                    <div className="text-xs text-muted-foreground">
                      还有 {draggedFiles.length - 5} 个文件...
                    </div>
                  )}
                </div>
              )}

              <p className="text-xs text-muted-foreground mt-2">
                松开鼠标以添加文件路径到输入
              </p>
            </div>
          </div>
        </div>
      )}

      {/* Error message */}
      {error && (
        <div className="absolute top-0 left-0 right-0 bg-red-500/90 text-white p-2 text-xs z-30 rounded-t">
          <span className="flex items-center gap-1">
            <span>⚠️</span>
            <span>{error}</span>
            <button
              onClick={() => setError(null)}
              className="ml-auto hover:text-white/80"
            >
              ✕
            </button>
          </span>
        </div>
      )}

      {/* Children */}
      {children}
    </div>
  );
}

// File path input helper
export function createFileInputPrompt(files: FileList): string {
  const fileNames = Array.from(files).map(f => f.name);
  if (fileNames.length === 1) {
    return `请处理文件: ${fileNames[0]}`;
  }
  return `请处理这些文件:\n${fileNames.map(f => `- ${f}`).join('\n')}`;
}

// Get file content as text (for supported text files)
export async function getFileContent(file: File): Promise<string | null> {
  if (!isFileSupported(file)) return null;

  try {
    return await file.text();
  } catch (e) {
    console.error('Failed to read file:', e);
    return null;
  }
}