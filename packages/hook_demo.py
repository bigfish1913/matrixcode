#!/usr/bin/env python3
"""
MatrixCode Hooks 功能演示
展示工具执行前后的拦截、修改和验证机制
"""

import json
from typing import Dict, List, Optional, Any
from enum import Enum
from dataclasses import dataclass

class HookResult(Enum):
    CONTINUE = "continue"
    BLOCK = "block"
    MODIFY = "modify"

@dataclass
class HookResponse:
    result: HookResult
    reason: Optional[str] = None
    details: Optional[str] = None
    modified_params: Optional[Dict] = None

class ToolHook:
    """工具执行 Hook 基类"""
    
    def __init__(self, name: str, enabled: bool = True):
        self.name = name
        self.enabled = enabled
        self.applies_to: List[str] = []  # 空列表表示应用到所有工具
    
    def applies_to_tool(self, tool_name: str) -> bool:
        """检查是否应用到特定工具"""
        return not self.applies_to or tool_name in self.applies_to
    
    def pre_execute(self, tool_name: str, params: Dict) -> HookResponse:
        """工具执行前调用"""
        return HookResponse(result=HookResult.CONTINUE)
    
    def post_execute(self, tool_name: str, params: Dict, result: str) -> str:
        """工具执行后调用"""
        return result

class LoggingHook(ToolHook):
    """日志 Hook - 记录所有工具执行"""
    
    def __init__(self):
        super().__init__("logging", enabled=True)
    
    def pre_execute(self, tool_name: str, params: Dict) -> HookResponse:
        print(f"🔍 [LOG] 准备执行: {tool_name}")
        print(f"📋 [LOG] 参数: {json.dumps(params, indent=2, ensure_ascii=False)}")
        return HookResponse(result=HookResult.CONTINUE)
    
    def post_execute(self, tool_name: str, params: Dict, result: str) -> str:
        print(f"✅ [LOG] 完成: {tool_name} (结果: {len(result)} 字节)")
        return result

class SecurityHook(ToolHook):
    """安全 Hook - 防止访问敏感文件"""
    
    BLOCKED_PATHS = [
        "/etc/passwd",
        "/etc/shadow",
        ".env",
        "credentials.json",
        "secret.key",
        "id_rsa",
    ]
    
    def __init__(self):
        super().__init__("security", enabled=True)
        self.applies_to = ["write", "edit", "read"]
    
    def pre_execute(self, tool_name: str, params: Dict) -> HookResponse:
        path = params.get("path", "")
        
        for blocked in self.BLOCKED_PATHS:
            if blocked in path:
                return HookResponse(
                    result=HookResult.BLOCK,
                    reason=f"🚫 安全拦截: 禁止访问敏感路径 '{blocked}'",
                    details="此路径被安全策略保护。如需访问，请联系管理员授权。"
                )
        
        return HookResponse(result=HookResult.CONTINUE)

class AutoFormatHook(ToolHook):
    """自动格式化 Hook - 格式化 JSON 内容"""
    
    def __init__(self):
        super().__init__("auto_format", enabled=True)
        self.applies_to = ["write"]
    
    def pre_execute(self, tool_name: str, params: Dict) -> HookResponse:
        content = params.get("content", "")
        
        # 尝试格式化 JSON
        try:
            data = json.loads(content)
            formatted = json.dumps(data, indent=2, ensure_ascii=False)
            
            if formatted != content:
                new_params = params.copy()
                new_params["content"] = formatted
                print(f"✨ [FORMAT] 已自动格式化 JSON")
                return HookResponse(
                    result=HookResult.MODIFY,
                    modified_params=new_params
                )
        except json.JSONDecodeError:
            pass
        
        return HookResponse(result=HookResult.CONTINUE)

class HookRegistry:
    """Hook 注册中心 - 管理所有 hooks"""
    
    def __init__(self):
        self.hooks: List[ToolHook] = []
    
    def register(self, hook: ToolHook):
        """注册 hook"""
        self.hooks.append(hook)
    
    def pre_execute(self, tool_name: str, params: Dict) -> HookResponse:
        """执行所有 pre-execute hooks"""
        current_params = params.copy()
        
        for hook in self.hooks:
            if not hook.enabled or not hook.applies_to_tool(tool_name):
                continue
            
            response = hook.pre_execute(tool_name, current_params)
            
            if response.result == HookResult.BLOCK:
                # 立即返回，阻止执行
                return response
            elif response.result == HookResult.MODIFY:
                # 更新参数，继续下一个 hook
                current_params = response.modified_params
        
        # 如果参数被修改，返回 MODIFY；否则返回 CONTINUE
        if current_params != params:
            return HookResponse(
                result=HookResult.MODIFY,
                modified_params=current_params
            )
        else:
            return HookResponse(result=HookResult.CONTINUE)
    
    def post_execute(self, tool_name: str, params: Dict, result: str) -> str:
        """执行所有 post-execute hooks"""
        current_result = result
        
        for hook in self.hooks:
            if hook.enabled and hook.applies_to_tool(tool_name):
                current_result = hook.post_execute(tool_name, params, current_result)
        
        return current_result

def main():
    """演示 MatrixCode Hooks 功能"""
    
    print("🧪 MatrixCode Hooks 功能演示")
    print("=" * 70)
    
    # 创建 Hook 注册中心
    registry = HookRegistry()
    
    # 注册多个 hooks
    registry.register(LoggingHook())
    registry.register(SecurityHook())
    registry.register(AutoFormatHook())
    
    print("\n📦 已注册 Hooks:")
    for i, hook in enumerate(registry.hooks, 1):
        applies = hook.applies_to if hook.applies_to else ["所有工具"]
        print(f"  {i}. {hook.name} - 适用于: {', '.join(applies)}")
    
    print("\n" + "=" * 70)
    
    # 测试场景 1: 正常执行
    print("\n📝 测试场景 1: 正常写入文件")
    print("-" * 70)
    
    params1 = {
        "path": "test.txt",
        "content": "Hello, MatrixCode!"
    }
    
    response1 = registry.pre_execute("write", params1)
    if response1.result == HookResult.CONTINUE:
        print("✅ 结果: Hook 允许执行")
        result1 = registry.post_execute("write", params1, "文件写入成功")
        print(f"📊 最终结果: {result1}")
    
    print("\n" + "=" * 70)
    
    # 测试场景 2: 安全拦截
    print("\n🔒 测试场景 2: 尝试访问敏感文件")
    print("-" * 70)
    
    params2 = {
        "path": ".env",
        "content": "SECRET=password123"
    }
    
    response2 = registry.pre_execute("write", params2)
    if response2.result == HookResult.BLOCK:
        print(f"❌ 结果: {response2.reason}")
        if response2.details:
            print(f"📝 详情: {response2.details}")
    
    print("\n" + "=" * 70)
    
    # 测试场景 3: 自动格式化
    print("\n✨ 测试场景 3: 自动格式化 JSON")
    print("-" * 70)
    
    params3 = {
        "path": "config.json",
        "content": '{"name":"test","version":"1.0","enabled":true}'
    }
    
    response3 = registry.pre_execute("write", params3)
    if response3.result == HookResult.MODIFY:
        print("🔄 结果: Hook 修改了参数")
        print(f"原始内容: {params3['content']}")
        print(f"修改后内容: {response3.modified_params['content']}")
    
    print("\n" + "=" * 70)
    
    # 测试场景 4: 多个 hooks 协作
    print("\n🎯 测试场景 4: 多个 hooks 协作 (日志 + 格式化)")
    print("-" * 70)
    
    params4 = {
        "path": "data.json",
        "content": '{"items":["a","b","c"],"count":3}'
    }
    
    response4 = registry.pre_execute("write", params4)
    print(f"Hook 处理结果: {response4.result.value}")
    
    if response4.result in [HookResult.CONTINUE, HookResult.MODIFY]:
        final_params = response4.modified_params or params4
        result4 = registry.post_execute("write", final_params, "文件写入成功")
        print(f"📊 最终结果: {result4}")
    
    print("\n" + "=" * 70)
    
    # 测试场景 5: 工具过滤
    print("\n🔍 测试场景 5: 工具过滤测试")
    print("-" * 70)
    
    # SecurityHook 只应用于 write/edit/read
    # 对 bash 工具应该不拦截
    params5 = {
        "path": ".env",
        "command": "cat .env"
    }
    
    response5 = registry.pre_execute("bash", params5)
    print(f"工具: bash, 路径: .env")
    print(f"结果: {response5.result.value} (SecurityHook 不应用于 bash)")
    
    print("\n" + "=" * 70)
    print("\n✅ 所有测试完成！")
    
    print("\n💡 Hooks 系统特性总结:")
    print("  1. 🔍 拦截能力 - pre_execute 可以阻止工具执行")
    print("  2. 🔄 修改能力 - 可以修改工具参数（如自动格式化）")
    print("  3. 📊 监控能力 - post_execute 可以记录和修改结果")
    print("  4. 🎯 精准过滤 - applies_to 控制应用到哪些工具")
    print("  5. 🧩 可扩展性 - 通过继承 ToolHook 创建自定义 Hook")
    print("  6. 🛡️ 安全防护 - 防止访问敏感文件和路径")

if __name__ == "__main__":
    main()