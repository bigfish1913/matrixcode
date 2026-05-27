---
name: git-commit
description: 生成规范的 Git 提交信息，遵循 Conventional Commits 规范
---

# Git 提交信息生成器

<objective>
帮助用户生成符合 Conventional Commits 规范的提交信息，确保提交历史清晰可追溯
</objective>

<process>
1. 使用 `bash` 工具执行 `git status` 和 `git diff --cached` 查看暂存的更改
2. 分析变更内容，确定合适的 type 和 scope
3. 生成简洁、准确的提交信息
4. 询问用户是否满意，或需要调整
5. 执行 `git commit -m "..."` 提交
</process>

帮助用户生成符合 Conventional Commits 规范的提交信息。

## 规范格式

```
<type>(<scope>): <subject>

<body>

<footer>
```

## 类型 (type)

- `feat`: 新功能
- `fix`: 修复 bug
- `docs`: 文档变更
- `style`: 代码格式（不影响代码运行的变动）
- `refactor`: 重构（既不是新增功能，也不是修改 bug）
- `perf`: 性能优化
- `test`: 增加测试
- `chore`: 构建过程或辅助工具的变动
- `revert`: 回滚
- `ci`: CI 配置文件和脚本的变动

## 工作流程

1. 使用 `bash` 工具执行 `git status` 和 `git diff --cached` 查看暂存的更改
2. 分析变更内容，确定合适的 type 和 scope
3. 生成简洁、准确的提交信息
4. 询问用户是否满意，或需要调整
5. 执行 `git commit -m "..."` 提交

## 示例

用户暂存了一个修复登录 bug 的更改：
```
fix(auth): 修复登录页面验证码不显示的问题

- 修复验证码组件未正确渲染的问题
- 添加验证码加载失败的错误提示

Closes #123
```

## 注意事项

- subject 使用祈使句，首字母小写，结尾不加句号
- body 解释"做了什么"和"为什么"，而非"怎么做"
- 如果是破坏性变更，在 footer 中添加 `BREAKING CHANGE:`