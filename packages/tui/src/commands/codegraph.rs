//! /codegraph command

use crate::commands::{Command, CommandContext};

pub struct CodeGraphCommand;

impl Command for CodeGraphCommand {
    fn name(&self) -> &'static str {
        "codegraph"
    }

    fn aliases(&self) -> &[&'static str] {
        &["cg", "codegraph status", "codegraph info"]
    }

    fn help(&self) -> Option<&'static str> {
        Some("Show CodeGraph index status (nodes, files, pending changes)")
    }

    fn execute(&self, ctx: &mut CommandContext, _args: &[&str]) {
        if let Some(status) = &ctx.app.codegraph_status {
            if status.initialized {
                let mut content = format!(
                    "📊 CodeGraph 状态:\n\n\
                    ✅ 已初始化\n\
                    • 文件数: {}\n\
                    • 节点数: {}\n\
                    • 边数: {}\n",
                    status.file_count,
                    status.node_count,
                    status.edge_count
                );

                // Show pending changes if available
                if let Some(ref pending) = status.pending_changes {
                    if let Some(obj) = pending.as_object() {
                        let added = obj.get("added").and_then(|v| v.as_u64()).unwrap_or(0);
                        let modified = obj.get("modified").and_then(|v| v.as_u64()).unwrap_or(0);
                        let removed = obj.get("removed").and_then(|v| v.as_u64()).unwrap_or(0);
                        let total = added + modified + removed;

                        if total > 0 {
                            content.push_str(&format!(
                                "\n⚠️ 待同步: {} 个文件\n\
                                • 新增: {}\n\
                                • 修改: {}\n\
                                • 删除: {}\n",
                                total, added, modified, removed
                            ));
                        } else {
                            content.push_str("\n✅ 索引已同步\n");
                        }
                    }
                }

                content.push_str("\n💡 命令:\n");
                content.push_str("• `/codegraph sync` - 手动同步索引\n");
                content.push_str("• `code_sync` 工具 - 在对话中同步\n");

                ctx.push_system(content);
            } else {
                ctx.push_system(
                    "📊 CodeGraph 状态:\n\n\
                    ❌ 未初始化\n\n\
                    CodeGraph 是一个代码知识图谱，提供:\n\
                    • 符号搜索 (函数、类、变量)\n\
                    • 调用关系分析 (callers/callees)\n\
                    • 变更影响分析\n\n\
                    初始化命令:\n\
                    ```bash\n\
                    codegraph init -i\n\
                    ```\n\
                    ".into(),
                );
            }
        } else {
            ctx.push_system(
                "📊 CodeGraph 状态:\n\n\
                ❓ 状态未知\n\n\
                可能原因:\n\
                • CodeGraph CLI 未安装\n\
                • 项目目录不包含代码\n\
                • Watcher 未启动\n\n\
                安装命令:\n\
                ```bash\n\
                npm install -g @bigfish/codegraph\n\
                ```\n\
                ".into(),
            );
        }
        ctx.auto_scroll();
    }
}