import React, { useState } from 'react';
import { useChatStore } from '../stores/chatStore';
import { useToastContext } from '../contexts/ToastContext';

interface LoopTaskDialogProps {
  onClose: () => void;
}

export function LoopTaskDialog({ onClose }: LoopTaskDialogProps) {
  const [prompt, setPrompt] = useState('');
  const [interval, setInterval] = useState('10m');
  const [maxCount, setMaxCount] = useState<number | undefined>(undefined);
  const sendMessage = useChatStore((s) => s.sendMessage);
  const toast = useToastContext();

  const handleSubmit = async () => {
    if (!prompt.trim()) {
      toast.addToast({ type: 'error', message: '请输入执行内容' });
      return;
    }

    // Parse interval (format: Ns, Nm, Nh, Nd)
    const intervalMatch = interval.match(/^(\d+)([smhd])$/);
    if (!intervalMatch) {
      toast.addToast({ type: 'error', message: '间隔格式错误，应为 Ns/Nm/Nh/Nd (如 5m, 30m, 2h, 1d)' });
      return;
    }

    const value = parseInt(intervalMatch[1]);
    const unit = intervalMatch[2];

    // Convert to seconds
    let intervalSeconds: number;
    switch (unit) {
      case 's': intervalSeconds = value; break;
      case 'm': intervalSeconds = value * 60; break;
      case 'h': intervalSeconds = value * 3600; break;
      case 'd': intervalSeconds = value * 86400; break;
      default: intervalSeconds = 600; // Default 10m
    }

    // Send loop command to backend
    const loopCommand = `/loop ${interval} ${prompt}`;
    await sendMessage(loopCommand);

    toast.addToast({
      type: 'success',
      message: `Loop任务已创建: 每${interval}执行 "${prompt}"`,
    });

    onClose();
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-card border shadow-lg rounded-lg max-w-md w-full p-6">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold">创建 Loop 任务</h2>
          <button
            onClick={onClose}
            className="text-muted-foreground hover:text-foreground"
          >
            ✕
          </button>
        </div>

        <div className="space-y-4">
          {/* Prompt input */}
          <div>
            <label className="block text-sm font-medium mb-1">
              执行内容
            </label>
            <input
              type="text"
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              placeholder="例如: check the deploy"
              className="w-full px-3 py-2 bg-background border rounded-md focus:outline-none focus:ring-2 focus:ring-primary"
            />
          </div>

          {/* Interval input */}
          <div>
            <label className="block text-sm font-medium mb-1">
              执行间隔
            </label>
            <input
              type="text"
              value={interval}
              onChange={(e) => setInterval(e.target.value)}
              placeholder="例如: 5m, 30m, 2h, 1d"
              className="w-full px-3 py-2 bg-background border rounded-md focus:outline-none focus:ring-2 focus:ring-primary"
            />
            <p className="text-xs text-muted-foreground mt-1">
              格式: Ns (秒), Nm (分钟), Nh (小时), Nd (天)
            </p>
          </div>

          {/* Max count (optional) */}
          <div>
            <label className="block text-sm font-medium mb-1">
              最大执行次数 (可选)
            </label>
            <input
              type="number"
              value={maxCount || ''}
              onChange={(e) => setMaxCount(e.target.value ? parseInt(e.target.value) : undefined)}
              placeholder="不填则无限循环"
              className="w-full px-3 py-2 bg-background border rounded-md focus:outline-none focus:ring-2 focus:ring-primary"
            />
          </div>

          {/* Examples */}
          <div className="text-xs text-muted-foreground bg-muted/30 p-3 rounded">
            <p className="font-medium mb-1">示例:</p>
            <ul className="space-y-1">
              <li>• <code>5m /babysit-prs</code> - 每5分钟检查PR</li>
              <li>• <code>30m check the deploy</code> - 每30分钟检查部署</li>
              <li>• <code>1h /standup 1</code> - 每小时执行standup</li>
            </ul>
          </div>
        </div>

        {/* Actions */}
        <div className="flex gap-3 mt-6">
          <button
            onClick={onClose}
            className="flex-1 px-4 py-2 bg-muted text-muted-foreground rounded-md hover:bg-muted/80"
          >
            取消
          </button>
          <button
            onClick={handleSubmit}
            className="flex-1 px-4 py-2 bg-primary text-primary-foreground rounded-md hover:bg-primary/90"
          >
            创建任务
          </button>
        </div>
      </div>
    </div>
  );
}