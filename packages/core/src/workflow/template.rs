//! Template Rendering Engine
//!
//! 简单的模板渲染引擎，支持 {{var}} 格式的变量替换。

use anyhow::{Context, Result};
use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

// Pre-compiled static regex patterns
static VAR_PATTERN: OnceLock<Regex> = OnceLock::new();
static NESTED_PATTERN: OnceLock<Regex> = OnceLock::new();
static DEFAULT_PATTERN: OnceLock<Regex> = OnceLock::new();

fn get_var_pattern() -> &'static Regex {
    VAR_PATTERN.get_or_init(|| Regex::new(r"\{\{(\w+)\}\}").unwrap())
}

fn get_nested_pattern() -> &'static Regex {
    NESTED_PATTERN.get_or_init(|| Regex::new(r"\{\{(\w+(?:\.\w+)+)\}\}").unwrap())
}

fn get_default_pattern() -> &'static Regex {
    DEFAULT_PATTERN.get_or_init(|| Regex::new(r"\{\{(\w+)\|([^}]+)\}\}").unwrap())
}

/// 模板渲染器
#[derive(Debug)]
pub struct TemplateRenderer;

impl Default for TemplateRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateRenderer {
    /// 创建新的模板渲染器
    pub fn new() -> Self {
        Self
    }

    /// 渲染模板字符串
    ///
    /// 支持以下语法：
    /// - `{{var}}` - 简单变量替换
    /// - `{{var.field}}` - 嵌套访问
    /// - `{{var|default}}` - 带默认值
    pub fn render(
        &self,
        template: &str,
        variables: &HashMap<String, serde_json::Value>,
    ) -> Result<String> {
        let mut result = template.to_string();

        // 先处理嵌套访问（更具体的模式）
        result = self.render_nested(&result, variables)?;

        // 处理带默认值的情况
        result = self.render_with_default(&result, variables)?;

        // 最后处理简单变量
        result = self.render_simple(&result, variables)?;

        Ok(result)
    }

    /// 渲染简单变量 {{var}}
    fn render_simple(
        &self,
        template: &str,
        variables: &HashMap<String, serde_json::Value>,
    ) -> Result<String> {
        let mut result = template.to_string();
        let var_pattern = get_var_pattern();

        for cap in var_pattern.captures_iter(template) {
            // Regex pattern guarantees capture groups exist, but handle edge cases gracefully
            let full_match = cap.get(0).map(|m| m.as_str()).unwrap_or_default();
            let var_name = cap.get(1).map(|m| m.as_str()).unwrap_or_default();

            if !full_match.is_empty() && !var_name.is_empty()
                && let Some(value) = variables.get(var_name) {
                    let replacement = self.value_to_string(value)?;
                    result = result.replace(full_match, &replacement);
                }
        }

        Ok(result)
    }

    /// 渲染嵌套访问 {{var.field}}
    fn render_nested(
        &self,
        template: &str,
        variables: &HashMap<String, serde_json::Value>,
    ) -> Result<String> {
        let mut result = template.to_string();
        let nested_pattern = get_nested_pattern();

        for cap in nested_pattern.captures_iter(template) {
            let full_match = cap.get(0).map(|m| m.as_str()).unwrap_or_default();
            let path = cap.get(1).map(|m| m.as_str()).unwrap_or_default();

            if !full_match.is_empty() && !path.is_empty()
                && let Some(value) = self.resolve_path(path, variables)? {
                    let replacement = self.value_to_string(&value)?;
                    result = result.replace(full_match, &replacement);
                }
        }

        Ok(result)
    }

    /// 渲染带默认值 {{var|default}}
    fn render_with_default(
        &self,
        template: &str,
        variables: &HashMap<String, serde_json::Value>,
    ) -> Result<String> {
        let mut result = template.to_string();
        let default_pattern = get_default_pattern();

        for cap in default_pattern.captures_iter(template) {
            let full_match = cap.get(0).map(|m| m.as_str()).unwrap_or_default();
            let var_name = cap.get(1).map(|m| m.as_str()).unwrap_or_default();
            let default_value = cap.get(2).map(|m| m.as_str()).unwrap_or_default();

            if !full_match.is_empty() && !var_name.is_empty() {
                let replacement = match variables.get(var_name) {
                    Some(value) => self.value_to_string(value)?,
                    None => default_value.to_string(),
                };
                result = result.replace(full_match, &replacement);
            }
        }

        Ok(result)
    }

    /// 解析路径（如 var.field.subfield）
    fn resolve_path(
        &self,
        path: &str,
        variables: &HashMap<String, serde_json::Value>,
    ) -> Result<Option<serde_json::Value>> {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.is_empty() {
            return Ok(None);
        }

        let first = parts[0];
        let mut current = variables.get(first).cloned();

        for part in parts.iter().skip(1) {
            match current {
                Some(serde_json::Value::Object(map)) => {
                    current = map.get(*part).cloned();
                }
                _ => return Ok(None),
            }
        }

        Ok(current)
    }

    /// 将 JSON 值转换为字符串
    fn value_to_string(&self, value: &serde_json::Value) -> Result<String> {
        match value {
            serde_json::Value::Null => Ok(String::new()),
            serde_json::Value::Bool(b) => Ok(b.to_string()),
            serde_json::Value::Number(n) => Ok(n.to_string()),
            serde_json::Value::String(s) => Ok(s.clone()),
            serde_json::Value::Array(arr) => {
                serde_json::to_string(arr).with_context(|| "Failed to stringify array")
            }
            serde_json::Value::Object(obj) => {
                serde_json::to_string(obj).with_context(|| "Failed to stringify object")
            }
        }
    }

    /// 从模板中提取所有变量名
    pub fn extract_variables(&self, template: &str) -> Vec<String> {
        let mut vars = Vec::new();
        let var_pattern = get_var_pattern();
        let nested_pattern = get_nested_pattern();
        let default_pattern = get_default_pattern();

        for cap in var_pattern.captures_iter(template) {
            vars.push(cap.get(1).unwrap().as_str().to_string());
        }

        for cap in nested_pattern.captures_iter(template) {
            let path = cap.get(1).unwrap().as_str();
            if let Some(first) = path.split('.').next() {
                vars.push(first.to_string());
            }
        }

        for cap in default_pattern.captures_iter(template) {
            vars.push(cap.get(1).unwrap().as_str().to_string());
        }

        vars.sort();
        vars.dedup();
        vars
    }

    /// 检查模板是否包含未解析的变量
    pub fn has_unresolved(&self, rendered: &str) -> bool {
        get_var_pattern().is_match(rendered)
            || get_nested_pattern().is_match(rendered)
            || get_default_pattern().is_match(rendered)
    }

    /// 渲染参数 HashMap
    ///
    /// 对每个参数值，如果是字符串则渲染模板，否则保持原值
    pub fn render_params(
        &self,
        params: &HashMap<String, serde_json::Value>,
        variables: &HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let mut rendered = HashMap::new();
        for (key, value) in params {
            let rendered_value = if let serde_json::Value::String(s) = value {
                let rendered_str = self.render(s, variables)?;
                serde_json::Value::String(rendered_str)
            } else {
                value.clone()
            };
            rendered.insert(key.clone(), rendered_value);
        }
        Ok(serde_json::Value::Object(rendered.into_iter().collect()))
    }
}

/// 便捷渲染函数
pub fn render(template: &str, variables: &HashMap<String, serde_json::Value>) -> Result<String> {
    let renderer = TemplateRenderer::new();
    renderer.render(template, variables)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_simple_variable() {
        let renderer = TemplateRenderer::new();
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), json!("Alice"));

        let result = renderer.render("Hello, {{name}}!", &vars).unwrap();
        assert_eq!(result, "Hello, Alice!");
    }

    #[test]
    fn test_multiple_variables() {
        let renderer = TemplateRenderer::new();
        let mut vars = HashMap::new();
        vars.insert("first".to_string(), json!("John"));
        vars.insert("last".to_string(), json!("Doe"));

        let result = renderer.render("{{first}} {{last}}", &vars).unwrap();
        assert_eq!(result, "John Doe");
    }

    #[test]
    fn test_nested_access() {
        let renderer = TemplateRenderer::new();
        let mut vars = HashMap::new();
        vars.insert(
            "user".to_string(),
            json!({
                "name": "Bob",
                "age": 30
            }),
        );

        let result = renderer
            .render("Name: {{user.name}}, Age: {{user.age}}", &vars)
            .unwrap();
        assert_eq!(result, "Name: Bob, Age: 30");
    }

    #[test]
    fn test_default_value() {
        let renderer = TemplateRenderer::new();
        let vars = HashMap::new();

        let result = renderer.render("Hello, {{name|Guest}}!", &vars).unwrap();
        assert_eq!(result, "Hello, Guest!");
    }

    #[test]
    fn test_extract_variables() {
        let renderer = TemplateRenderer::new();
        let template = "{{name}} is {{age}} years old, user: {{user.name}}";

        let vars = renderer.extract_variables(template);
        assert!(vars.contains(&"name".to_string()));
        assert!(vars.contains(&"age".to_string()));
        assert!(vars.contains(&"user".to_string()));
    }

    #[test]
    fn test_number_value() {
        let renderer = TemplateRenderer::new();
        let mut vars = HashMap::new();
        vars.insert("count".to_string(), json!(42));

        let result = renderer.render("Count: {{count}}", &vars).unwrap();
        assert_eq!(result, "Count: 42");
    }
}
