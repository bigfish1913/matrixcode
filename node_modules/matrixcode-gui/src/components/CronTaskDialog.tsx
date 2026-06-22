import React, { useState } from 'react';
import { useChatStore } from '../stores/chatStore';
import { useToastContext } from '../contexts/ToastContext';

interface CronTaskDialogProps {
  onClose: () => void;
}

export function CronTaskDialog({ onClose }: CronTaskDialogProps) {
  const [prompt, setPrompt] = useState('');
  const [cronExpression, setCronExpression] = useState('*/10 * * * *');
  const [durable, setDurable] = useState(false);
  const sendMessage = useChatStore((s) => s.sendMessage);
  const toast = useToastContext();

  // Parse cron expression helper
  const parseCronDescription = (cron: string): string => {
    const parts = cron.split(' ');
    if (parts.length !== 5) return 'Invalid cron expression';

    const [minute, hour, dayOfMonth, month, dayOfWeek] = parts;

    // Simple descriptions for common patterns
    if (minute.startsWith('*/')) {
      const interval = minute.slice(2);
      return `Every ${interval} minutes`;
    }
    if (hour.startsWith('*/') && minute === '0') {
      const interval = hour.slice(2);
      return `Every ${interval} hours`;
    }
    if (dayOfMonth.startsWith('*/') && minute === '0' && hour === '0') {
      const interval = dayOfMonth.slice(2);
      return `Every ${interval} days`;
    }
    if (minute === '0' && hour === '9' && dayOfMonth === '*' && month === '*' && dayOfWeek === '1-5') {
      return 'Every weekday at 9am';
    }

    return `Custom: ${cron}`;
  };

  const handleSubmit = async () => {
    if (!prompt.trim()) {
      toast.addToast({ type: 'error', message: '请输入执行内容' });
      return;
    }

    // Validate cron expression (basic check)
    const parts = cronExpression.split(' ');
    if (parts.length !== 5) {
      toast.addToast({ type: 'error', message: 'Cron表达式格式错误，需要5个字段' });
      return;
    }

    // Send cron command to backend
    const cronCommand = `/cron add "${cronExpression}" "${prompt}"${durable ? ' --durable' : ''}`;
    await sendMessage(cronCommand);

    toast.addToast({
      type: 'success',
      message: `Cron任务已创建: ${parseCronDescription(cronExpression)}执行 "${prompt}"`,
    });

    onClose();
  };

  // Quick templates
  const templates = [
    { label: 'Every 5 minutes', cron: '*/5 * * * *' },
    { label: 'Every 30 minutes', cron: '*/30 * * * *' },
    { label: 'Every hour', cron: '0 * * * *' },
    { label: 'Every 2 hours', cron: '0 */2 * * *' },
    { label: 'Every day at 9am', cron: '0 9 * * *' },
    { label: 'Every weekday at 9am', cron: '0 9 * * 1-5' },
    { label: 'Every Monday at 10am', cron: '0 10 * * 1' },
  ];

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div className="bg-card border shadow-lg rounded-lg max-w-lg w-full p-6">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold">创建 Cron 定时任务</h2>
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

          {/* Cron expression input */}
          <div>
            <label className="block text-sm font-medium mb-1">
              Cron 表达式
            </label>
            <input
              type="text"
              value={cronExpression}
              onChange={(e) => setCronExpression(e.target.value)}
              placeholder="*/10 * * * *"
              className="w-full px-3 py-2 bg-background border rounded-md focus:outline-none focus:ring-2 focus:ring-primary font-mono"
            />
            <p className="text-xs text-muted-foreground mt-1">
              格式: minute hour dayOfMonth month dayOfWeek
            </p>
            {/* Live preview */}
            <div className="mt-2 px-3 py-2 bg-muted/30 rounded text-sm">
              <span className="text-muted-foreground">预览: </span>
              <span className="text-primary">{parseCronDescription(cronExpression)}</span>
            </div>
          </div>

          {/* Quick templates */}
          <div>
            <label className="block text-sm font-medium mb-2">
              快速模板
            </label>
            <div className="grid grid-cols-2 gap-2">
              {templates.map((template) => (
                <button
                  key={template.cron}
                  onClick={() => setCronExpression(template.cron)}
                  className="px-3 py-2 text-xs bg-muted hover:bg-accent rounded border transition-colors"
                >
                  {template.label}
                </button>
              ))}
            </div>
          </div>

          {/* Durable option */}
          <div className="flex items-center gap-2">
            <input
              type="checkbox"
              id="durable"
              checked={durable}
              onChange={(e) => setDurable(e.target.checked)}
              className="rounded"
            />
            <label htmlFor="durable" className="text-sm">
              持久化任务 (重启后保留)
            </label>
          </div>

          {/* Examples */}
          <div className="text-xs text-muted-foreground bg-muted/30 p-3 rounded">
            <p className="font-medium mb-1">示例:</p>
            <ul className="space-y-1">
              <li>• <code>*/5 * * * *</code> - 每5分钟</li>
              <li>• <code>0 */2 * * *</code> - 每2小时</li>
              <li>• <code>0 9 * * 1-5</code> - 每个工作日9点</li>
              <li>• <code>30 14 28 2 *</code> - 2月28日14:30</li>
            </ul>
          </div>

          {/* Cron format reference */}
          <div className="text-xs text-muted-foreground bg-muted/30 p-3 rounded">
            <p className="font-medium mb-1">Cron 表达式格式:</p>
            <table className="w-full mt-2">
              <tbody>
                <tr className="border-b">
                  <td className="py-1 font-medium">字段</td>
                  <td className="py-1">范围</td>
                  <td className="py-1">说明</td>
                </tr>
                <tr className="border-b">
                  <td className="py-1">Minute</td>
                  <td className="py-1">0-59</td>
                  <td className="py-1">分钟</td>
                </tr>
                <tr className="border-b">
                  <td className="py-1">Hour</td>
                  <td className="py-1">0-23</td>
                  <td className="py-1">小时</td>
                </tr>
                <tr className="border-b">
                  <td className="py-1">Day of Month</td>
                  <td className="py-1">1-31</td>
                  <td className="py-1">日期</td>
                </tr>
                <tr className="border-b">
                  <td className="py-1">Month</td>
                  <td className="py-1">1-12</td>
                  <td className="py-1">月份</td>
                </tr>
                <tr>
                  <td className="py-1">Day of Week</td>
                  <td className="py-1">0-6</td>
                  <td className="py-1">星期 (0=周日)</td>
                </tr>
              </tbody>
            </table>
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