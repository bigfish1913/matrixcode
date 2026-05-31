// This file is used to store Chinese prompts for focus extraction
// 将在 focus_extractor.rs 中使用

pub const EXTRACTION_PROMPT: &str = r#"分析对话内容并提取聚焦点。

对话内容：
{conversation}

提取规则：
1. 识别用户正在处理的主要主题/任务
2. 为每个主题提取相关关键词和实体
3. 判断是否存在多个并行焦点或任务切换
4. 识别每个焦点的核心问题或目标

输出 JSON 格式：
{
  "focuses": [
    {
      "topic": "焦点简述",
      "keywords": ["关键词1", "关键词2"],
      "entities": ["文件名.rs", "函数名", "模块名"],
      "core_question": "用户想要实现什么",
      "importance": 0.8,
      "is_current": true
    }
  ],
  "focus_transitions": [
    {
      "from": "主题1",
      "to": "主题2",
      "trigger": "用户说'但是'",
      "message_index": 5
    }
  ]
}

焦点提取规则：
- 技术术语、文件名、函数名 → entities
- 领域概念、工具、框架 → keywords
- 用户明确提出的问题 → core_question
- 活跃任务 → importance > 0.7
- 暂停任务 → importance < 0.5
- 当前焦点 → is_current: true
"#;

pub const CLASSIFICATION_PROMPT: &str = r#"判断用户输入属于哪个聚焦点。

用户输入：
{user_input}

现有聚焦点：
{foci_description}

判断规则：
1. 计算每个现有焦点的相关性分数（0.0-1.0）
2. 判断输入是否开启新焦点
3. 如果是新焦点，提取其主题、关键词和实体

输出 JSON：
{
  "classification": {
    "matched_focus_id": "focus-123" 或 null,
    "relevance_scores": {
      "focus-1": 0.85,
      "focus-2": 0.12
    },
    "is_new_focus": false,
    "reason": "输入提到'database'，匹配 focus-1 的关键词"
  },
  "new_focus": null 或 {
    "topic": "新主题描述",
    "keywords": ["新关键词"],
    "entities": ["新实体"],
    "core_question": "新问题"
  }
}

分类规则：
- 关键词/实体匹配 → 高相关性
- 主题相似 → 中相关性
- 完全不同 → 低相关性（< 0.3）→ 新焦点
- 用户明确切换话题 → 新焦点
"#;

pub const SUMMARY_PROMPT: &str = r#"请简洁总结以下对话片段。

重点关注：
1. 做出的关键决策
2. 发现的重要事实
3. 使用的工具及其结果
4. 遇到的问题和解决方案

对话内容：
---
{conversation}
---

摘要："#;