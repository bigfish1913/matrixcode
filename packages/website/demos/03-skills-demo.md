# MatrixCode Skills 技能调用 Demo

Skills 是 MatrixCode 的核心特性，提供场景化的最佳实践指导。

## 🎯 Demo 目标

展示如何：
1. 理解 Skills 系统
2. 调用已注册的 Skills
3. 创建自定义 Skill

## 📋 Skills 系统概述

### Skills 分类

| 类型 | 说明 | 优先级 | 示例 |
|------|------|--------|------|
| **Process Skills** | 流程指导技能 | 高（优先调用） | om:debug, om:brainstorm, om:plan |
| **Implementation Skills** | 实现指导技能 | 中 | om:feature, om:test, refactor |

### Skills 类型（执行严格度）

| 类型 | 说明 | 示例 |
|------|------|------|
| **Rigid** | 必须严格遵循 | TDD, debugging（不能跳过步骤） |
| **Flexible** | 可适应调整 | patterns, style guides |

## 🚀 使用流程

### 1. 自动触发（推荐）

MatrixCode 会根据用户输入自动识别并加载合适的 Skill。

**用户输入：**
```
我遇到了一个bug，程序崩溃了，帮我调试
```

**MatrixCode 响应：**
```
💭 Thinking: 分析意图...
   → 用户遇到技术问题
   → 需要系统诊断
   → 匹配 om:debug skill

🔧 Skill: om:debug
加载调试技能...

<command-name>om:debug</command-name>

## Debugging 工作流

1. **复现问题**: 理解错误信息和触发条件
2. **定位代码**: 使用工具查找相关代码
3. **验证假设**: 用日志/调试器验证根因
4. **修复问题**: 修复根本原因而非表面补丁
5. **验证修复**: 运行测试确认

开始执行第 1 步...

🔍 分析错误信息...
   Error: panic at src/main.rs:42
   Message: "index out of bounds"

📖 read: src/main.rs:42
找到问题代码...

✅ 已定位：数组访问越界

继续第 3 步...
```

### 2. 手动触发（使用 /skill 命令）

**用户输入：**
```
/review
```

**MatrixCode 响应：**
```
🔧 Skill: code-review
加载代码审查技能...

执行代码审查流程...
```

### 3. Skill 直接调用（高级用户）

**用户输入：**
```
使用 skill 工具加载 refactor skill
```

**MatrixCode 响应：**
```
🔧 tool: skill
参数: { name: "refactor" }

加载 refactor skill...

<command-name>refactor</command-name>

## Refactor 指导原则

1. 保持功能不变
2. 最小改动范围
3. 清晰命名意图
4. 单一职责原则

请告诉我需要重构什么代码？
```

## 📊 内置 Skills 示例

### om:debug - 问题调试 Skill

**触发条件：**
- 用户说 "调试问题"、"遇到了bug"、"程序崩溃"
- 用户询问失败原因（"为什么失败？")

**工作流程（Rigid）：**
```
1. 复现问题 → 理解错误、场景、触发条件
2. 定位代码 → 搜索符号、查找调用、读取文件
3. 验证假设 → 使用工具（日志、调试器）验证
4. 修复问题 → 修复根本原因
5. 验证修复 → 运行测试
```

**示例对话：**
```
用户: "我的 Rust 程序报错了：thread 'main' has overflowed its stack"

Agent: 
[om:debug skill loaded]

第 1 步：复现问题
✅ 错误类型：栈溢出
✅ 触发位置：递归函数

第 2 步：定位代码
🔍 code_search: 查找递归函数...
📖 read: src/recursive.rs
找到问题函数：factorial(n) { factorial(n-1) }  # 无终止条件

第 3 步：验证假设
✅ 假设正确：无限递归导致栈溢出

第 4 步：修复问题
✏️ edit: src/recursive.rs
添加终止条件：
fn factorial(n: u64) -> u64 {
    if n <= 1 { 1 }
    else { n * factorial(n-1) }
}

第 5 步：验证修复
🔧 bash: cargo test
✅ 所有测试通过

✅ 调试完成！
```

### om:plan - 规划 Skill

**触发条件：**
- 用户说 "规划方案"、"设计架构"、"先计划一下"
- 用户描述复杂任务，需要先制定计划

**工作流程：**
```
1. 理解需求 → 分析目标、功能、约束
2. 查看现有代码 → 搜索相关模块、分析依赖
3. 设计方案 → 制定架构、模块划分
4. 输出计划 → plan.md + tasks-input.json
```

**示例对话：**
```
用户: "我需要实现一个用户认证系统，包含登录、注册、权限管理"

Agent:
[om:plan skill loaded]

第 1 步：理解需求
✅ 功能：登录、注册、权限管理
✅ 约束：安全、可扩展

第 2 步：查看现有代码
📁 ls: 查看项目结构...
🔍 code_search: 查找现有认证逻辑...

第 3 步：设计方案
推荐架构：
- auth模块：认证核心逻辑
- models/user.rs：用户模型
- middleware/auth.rs：权限中间件
- handlers/auth.rs：HTTP接口

第 4 步：输出计划
✏️ write: plan.md
✏️ write: tasks-input.json

✅ 规划完成！计划文件已生成。
下一步：运行 /om:start 开始实施
```

### om:test - 测试生成 Skill

**触发条件：**
- 用户说 "生成测试"、"写测试代码"、"增加测试覆盖"

**工作流程：**
```
1. 分析代码 → 查找需要测试的函数
2. 生成测试 → 创建测试文件
3. 运行测试 → 验证测试通过
4. 修复失败 → 如果失败，循环修复（最多3次）
```

**示例对话：**
```
用户: "给 src/calc.rs 生成测试"

Agent:
[om:test skill loaded]

第 1 步：分析代码
📖 read: src/calc.rs
找到函数：add(), subtract(), multiply()

第 2 步：生成测试
✏️ write: src/calc_test.rs
#[test]
fn test_add() { assert_eq!(add(2, 3), 5); }
#[test]
fn test_subtract() { assert_eq!(subtract(5, 3), 2); }

第 3 步：运行测试
🔧 bash: cargo test
✅ test_add passed
✅ test_subtract passed

✅ 测试生成完成！覆盖率：100%
```

## 🎯 创建自定义 Skill

### Skill 文件格式

```markdown
---
name: my-custom-skill
description: 自定义技能说明
trigger: 用户触发条件描述
priority: implementation
type: flexible
---

# Skill 标题

## 何时使用
- 条件1
- 条件2

## 执行步骤
1. 步骤1
2. 步骤2

## 最佳实践
- 建议1
- 建议2
```

### 创建示例 Skill

**文件：.skills/my-skill/SKILL.md**

```markdown
---
name: api-design
description: RESTful API 设计指导
trigger: 用户需要设计 API 接口
priority: implementation
type: flexible
---

# RESTful API 设计 Skill

## 何时使用
- 用户说 "设计 API"、"创建接口"
- 用户描述需要暴露的功能

## 设计原则

1. **资源命名**
   - 使用名词而非动词
   - 使用复数形式：/users, /posts

2. **HTTP 方法**
   - GET: 获取资源
   - POST: 创建资源
   - PUT: 更新资源
   - DELETE: 删除资源

3. **状态码**
   - 200: 成功
   - 201: 创建成功
   - 400: 请求错误
   - 404: 资源不存在

4. **版本控制**
   - URL版本：/api/v1/users
   - Header版本：Accept: application/vnd.api.v1+json

## 执行流程

1. 理解需求 → 分析资源和方法
2. 设计路由 → 规划 URL 结构
3. 定义响应 → 设计 JSON 格式
4. 编写代码 → 实现路由处理器
5. 添加测试 → 验证 API 功能

## 代码示例

```rust
// GET /api/v1/users/:id
async fn get_user(id: u64) -> Result<User, Error> {
    User::find(id)
}

// POST /api/v1/users
async fn create_user(data: NewUser) -> Result<User, Error> {
    User::create(data)
}
```
```

### 注册 Skill

将 Skill 文件放到 `.skills/` 目录，MatrixCode 会自动发现和加载。

## ✨ Skills 系统的优势

1. **场景化指导**: 每个 Skill 针对特定场景提供最佳实践
2. **流程标准化**: Rigid Skills 确保关键流程不被跳过
3. **可扩展性**: 用户可创建自定义 Skills
4. **自动触发**: AI 自动识别并加载合适的 Skill
5. **低 Token 消耗**: 只在需要时加载完整内容

## 🔗 相关文档

- [Skills 系统详解](../docs.html)
- [创建自定义 Skill](../docs.html#custom-skills)
- [内置 Skills 列表](../docs.html#builtin-skills)

## 📊 测试验证

运行以下命令验证 Skills 系统：

```bash
# 查看已注册 Skills
matrixcode --list-skills

# 测试 Skill 加载
matrixcode
> /debug  # 测试 om:debug skill
> /plan   # 测试 om:plan skill

# 查看自定义 Skill
ls .skills/
```

预期输出：
```
Available skills:
  - om:debug (process, rigid)
  - om:plan (process, flexible)
  - om:test (implementation, rigid)
  - refactor (implementation, flexible)
  - my-custom-skill (implementation, flexible)
```