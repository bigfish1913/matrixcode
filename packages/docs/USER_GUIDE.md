# MatrixCode 用户使用指南

**适用版本**: v0.4.7+  
**更新日期**: 2025年1月22日

---

## 🚀 快速开始

### 安装和配置

```bash
# 1. 安装MatrixCode
git clone https://github.com/your-org/matrixcode
cd matrixcode

# 2. 配置环境
cargo build --release

# 3. 查看配置
matrix config show

# 4. 设置审批模式（建议生产环境）
matrix config set approve_mode ask
```

---

## 🔒 安全使用指南

### 理解安全边界

**MatrixCode的安全机制**:
```
4层防护确保安全：
第1层: bash命令黑名单（阻止30+危险命令）
第2层: 路径完整验证（阻止穿越和系统文件）
第3层: 内容大小限制（最大10MB）
第4层: 迭代次数保护（最大50次）

⚠️ 注意：这不是沙箱，而是防护机制
✅ 目的：防止明显的灾难性操作
✅ 用户仍需谨慎使用和审查操作
```

---

## 📋 常用操作示例

### 文件操作（安全示例）

**✅ 正确使用**:
```bash
# 写入项目文件（相对路径）
write("./src/main.rs", "fn main() { println!(\"Hello\"); }")
write("config.json", "{\"version\": \"1.0\"}")

# 读取文件
read("./README.md")
cat config.json

# 删除构建目录（安全路径）
rm -rf ./build
rm -rf /tmp/test-cache  # 临时目录安全

# 创建目录
mkdir src
mkdir -p lib/modules
```

**❌ 错误使用（会被阻止）**:
```bash
# 这些操作会被安全机制阻止：

# 1. 系统破坏命令
rm -rf /                  # ❌ 阻止：删除根目录
mkfs                      # ❌ 阻止：格式化磁盘
shutdown                  # ❌ 阻止：关机命令

# 2. 路径穿越
write("../../etc/passwd") # ❌ 阻止：路径穿越
rm -rf ../../../var       # ❌ 阻止：穿越删除

# 3. 系统文件写入
write("/etc/passwd")      # ❌ 阻止：系统文件
write("/etc/shadow")      # ❌ 阻止：密码文件

# 4. 超大内容
write("large.txt", "11MB内容") # ❌ 阻止：超过10MB
```

---

## 🎯 最佳实践

### 1. 使用相对路径

**建议**:
```bash
# ✅ 推荐：使用相对路径（项目内操作）
write("./src/module.rs", code)
edit("./config.json", updates)
read("./docs/README.md")

# ⚠️ 避免：绝对路径（可能误触系统文件）
write("/usr/local/file.txt", data)  # 不推荐

# ❌ 禁止：路径穿越（会被阻止）
write("../../etc/file", data)  # 自动阻止
```

### 2. 分解复杂任务

**建议**:
```bash
# ✅ 推荐：分解为小任务
"重构auth模块"  # 小任务，可在50次迭代内完成
"重构database模块"
"重构api模块"

# ⚠️ 避免：过大任务（可能达到MAX_ITERATIONS）
"重构整个系统架构"  # 太大，可能达到50次上限
```

**达到上限怎么办**:
```
如果看到警告：
⚠️ Reached maximum iterations limit (50 iterations).

解决方案：
1. 检查已完成部分
2. 继续剩余任务（新对话）
3. 使用更具体的指令
4. 分解为多个小任务
```

### 3. 大文件处理建议

**小文件（<1MB）**:
```bash
# 直接写入即可
write("config.json", data)  # 快速完成
```

**中等文件（1-5MB）**:
```bash
# 会收到大文件提示
write("data.json", "2MB数据")
# 提示：(2.00 MB - large file written successfully. Consider splitting)

# 建议：如果性能有问题，考虑拆分
```

**大文件（>10MB）**:
```bash
# ❌ 会被阻止
write("huge.txt", "11MB内容")
# 错误：Content too large (max: 10MB)

# ✅ 解决方案：分批写入
write("part1.txt", "第一部分5MB")
write("part2.txt", "第二部分5MB")
write("part3.txt", "第三部分")
```

### 4. 安全临时目录使用

**安全临时目录**:
```bash
# ✅ 允许：安全的临时目录
rm -rf /tmp/test-cache     # 安全
rm -rf /var/tmp/build      # 安全
write("/tmp/output.txt")    # 安全（但文档说明风险）

# ⚠️ 注意：临时文件重启后可能消失
# 建议：重要文件放在项目目录内
```

---

## ⚠️ 常见错误处理

### 错误1: 命令被阻止

**现象**:
```
Error: destructive or dangerous command blocked
Error: destructive rm -rf on root path blocked
```

**原因**: 输入了危险命令

**解决**:
```bash
# 1. 查看错误消息理解原因
# 2. 使用安全的替代方案

# 例如：想删除构建目录
错误: rm -rf /
正确: rm -rf ./build

# 例如：想清理临时文件
错误: rm -rf /*
正确: rm -rf /tmp/my-cache
```

### 错误2: 路径穿越被阻止

**现象**:
```
Error: Path traversal detected: '../../etc/passwd'
Paths cannot contain '..' for security
```

**原因**: 路径包含穿越符号 `..`

**解决**:
```bash
# 1. 使用项目相对路径（无穿越）
错误: write("../../etc/passwd")
正确: write("./config/passwd.conf")

# 2. 确认目标路径安全性
# 3. 如确需访问上级目录，手动执行并确认风险
```

### 错误3: 系统文件写入被阻止

**现象**:
```
Error: Cannot write to critical system file: '/etc/passwd'
This is blocked for security
```

**原因**: 尝试写入系统关键文件

**解决**:
```bash
# 1. 理解为什么被��止（保护系统安全）
# 2. 使用替代方案

# 例如：修改hosts配置
错误: write("/etc/hosts", "新配置")
正确方案:
  step1: write("./hosts.conf", "配置内容")
  step2: 手动执行（确认风险后）
         sudo cp ./hosts.conf /etc/hosts
```

### 错误4: 内容过大被阻止

**现象**:
```
Error: Content too large: 11,000,000 bytes (max: 10,000,000 bytes = 10 MB)
Split into smaller files or use streaming
```

**原因**: 单次写入内容超过10MB限制

**解决**:
```bash
# 分批写入
write("part1.txt", "第一部分（5MB）")
write("part2.txt", "第二部分（5MB）")
write("part3.txt", "剩余部分")

# 或者压缩后再写入
compress large_data.txt
write("compressed.zip", compressed_data)
```

### 错误5: 达到迭代上限

**现象**:
```
⚠️ Reached maximum iterations limit (50 iterations).
Task may not be fully complete.
```

**原因**: 任务复杂度过高，Agent执行了50次迭代

**解决**:
```bash
# 选项1: 检查已完成部分，继续剩余任务
"检查auth模块重构状态"
"继续重构auth模块剩余部分"

# 选项2: 分解为更小的任务
错误任务: "重构整个系统"
正确任务: "重构auth模块"（更小）

# 选项3: 使用更具体的指令
模糊指令: "优化代码"
具体指令: "优化auth模块的错误处理逻辑"
```

---

## 🔧 配置建议

### 开发环境配置

```yaml
# 快速开发，最小审批
approve_mode: auto    # 自动执行（安全限制仍生效）

# 开发环境特点：
# - 快速迭代
# - 安全限制保护
# - 无审批延迟
```

### 生产环境配置

```yaml
# 严格审批，每次确认
approve_mode: ask     # 每个操作都询问确认

# 生产环境特点：
# - 严格审查
# - 用户确认每个操作
# - 最大安全性
```

### 学习环境配置

```yaml
# 温和提醒，不强制
approve_mode: suggest # 建议但不强制

# 学习环境特点：
# - 学习安全边界
# - 看到建议但不强制
# - 理解阻止原因
```

---

## 📚 进阶使用技巧

### 技巧1: 批量文件操作

```bash
# 批量创建文件（避免达到迭代上限）
"创建5个模块文件：auth.rs, db.rs, api.rs, utils.rs, config.rs"
# Agent会在一次操作中批量处理

# 而不是：
"创建auth.rs"
"创建db.rs"
...  # 逐个创建（可能达到上限）
```

### 技巧2: 使用模板加速

```bash
# 提供模板减少迭代次数
write("./src/auth.rs", """
// Auth module template
pub struct AuthModule {
    config: Config,
}

impl AuthModule {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}
""")

# 模板减少后续修改次数
```

### 技巧3: 精确指令减少迭代

```bash
# 精确指令（减少迭代）
"在auth.rs第10行添加错误处理函数error_handler()"

# 模糊指令（增加迭代）
"改进auth模块"
# Agent需要多次尝试才能理解需求
```

---

## 🆘 故障排查

### 问题1: 操作被阻止但认为应该允许

**排查步骤**:
```bash
1. 查看SECURITY.md了解安全边界
2. 检查错误消息中的具体原因
3. 确认是否在白名单路径内
4. 确认命令是否在黑名单中

如果确信应该允许：
- 检查是否拼写错误
- 检查路径格式是否正确
- 查看文档中的允许/阻止清单
```

### 问题2: 任务一直不完成

**排查步骤**:
```bash
1. 检查是否达到MAX_ITERATIONS（50次）
2. 如果达到上限，查看警告消息
3. 分解任务为更小的部分
4. 使用更具体的指令
5. 检查是否有循环依赖或死循环
```

### 问题3: 文件写入失败

**排查步骤**:
```bash
1. 检查路径是否包含穿越（../）
2. 检查是否是系统文件路径
3. 检查内容大小是否超过10MB
4. 检查路径长度是否超过1024字符
5. 检查是否有权限问题（非MatrixCode问题）
```

---

## 📖 学习���源

### 文档阅读顺序

**新手用户**:
```
1. README.md - 项目介绍
2. SECURITY.md - 安全边界说明
3. 本使用指南 - 实践指导
```

**开发者**:
```
1. SECURITY.md - 安全机制详解
2. SECURITY_TEST_PLAN.md - 测试验证方法
3. IMPROVEMENT_COMPLETE.md - 改进经验学习
```

**审计员**:
```
1. CODE_REVIEW_REPORT.md - 完整审查报告
2. SECURITY.md - 安全边界文档
3. SECURITY_TEST_PLAN.md - 验证测试计划
```

---

## 💬 用户反馈和建议

### 反馈渠道

```bash
# GitHub Issues
https://github.com/your-org/matrixcode/issues

# 安全问题
请私下联系开发者（不公开披露）

# 功能建议
创建Issue讨论需求和实现方案
```

### 常见反馈类型

**安全建议**:
```
"建议添加更多黑名单命令"
"建议提供用户可配置限制"
"建议添加安全审计日志"
```

**体验建议**:
```
"错误消息很详细，很有帮助"
"希望添加实时进度条"
"希望支持更多文件类型"
```

**文档建议**:
```
"文档很完整，快速上手"
"建议添加更多使用示例"
"建议添加视频教程"
```

---

## ✅ 使用检查清单

### 新手用户检查清单

```
✅ 已阅读README.md了解项目
✅ 已阅读SECURITY.md了解安全边界
��� 已配置approve_mode（建议ask或suggest）
✅ 知道如何查看错误消息
✅ 知道基本的允许/阻止操作
✅ 知道如何分解任务避免达到上限
✅ 知道如何使用相对路径
```

### 开发者检查清单

```
✅ 已阅读完整SECURITY.md文档
✅ 理解4层安全防护机制
✅ 知道如何测试安全功能
✅ 知道如何调试阻止问题
✅ 知道如何优化任务减少迭代
✅ 知道配置选项和最佳实践
```

---

## 🎯 快速参考卡

### 允许的操作 ✅

```bash
文件操作:
✅ write("./src/file.rs", code)     # 项目文件
✅ read("./README.md")              # 读取文件
✅ rm -rf ./build                   # 删除构建目录
✅ rm -rf /tmp/test                 # 安全临时目录
✅ mkdir src                        # 创建目录

构建操作:
✅ cargo build                      # 构建项目
✅ npm install                      # 安装依赖
✅ make                             # 编译项目

开发工具:
✅ git status                       # Git操作
✅ cat file.txt                     # 读取文件
✅ grep pattern file                # 搜索内容
```

### 阻止的操作 ❌

```bash
系统破坏:
❌ rm -rf /                         # 删除根目录
❌ mkfs                             # 格式化磁盘
❌ shutdown                         # 关机命令
❌ :(){:|:&};:                      # Fork炸弹

路径穿越:
❌ write("../../etc/passwd")        # 路径穿越
❌ rm -rf ../../../var              # 穿越删除

系统文件:
❌ write("/etc/passwd")             # 系统文件
❌ write("/etc/shadow")             # 密码文件

超大操作:
❌ write("huge.txt", "11MB")        # 超过10MB
```

---

**🎉 使用MatrixCode安全高效地完成开发任务！**

**记住**:
- ✅ 使用相对路径（项目内操作）
- ✅ 分解复杂任务（避免达到上限）
- ✅ 查看详细错误消息（理解原因）
- ✅ 阅读SECURITY.md（了解边界）
- ✅ 配置合适模式（平衡安全效率）

---

**版本**: v0.4.7  
**更新**: 2025年1月22日  
**文档**: 完整使用指南