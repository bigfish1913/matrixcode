---
name: example-skill
description: 一个示例 skill，演示如何编写技能
---

# 示例技能

这是一个示例 skill，用于演示 MatrixCode 的技能系统。

## 用途

当用户请求与该技能相关的任务时，模型会：
1. 自动识别技能名称和描述
2. 使用 `skill` 工具加载完整的技能指令
3. 按照指令执行任务

## 使用方法

你可以在这个目录下添加任何辅助文件：
- 脚本文件（如 helper.sh）
- 模板文件（如 templates/）
- 参考文档（如 reference.md）

模型可以使用 `read` 工具读取这些文件。

## 示例

例如，如果你要创建一个代码审查 skill：

```
skills/
  code-review/
    SKILL.md
    checkstyle.py
    templates/
      review-template.md
```

然后在 SKILL.md 中编写审查指南。