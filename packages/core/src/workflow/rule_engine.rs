//! Rule Engine for Workflow Validation
//!
//! 验证规则引擎，支持条件表达式解析和验证。

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 验证规则
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Rule {
    /// 包含检查
    Contains {
        field: String,
        value: String,
        #[serde(default)]
        case_sensitive: bool,
    },
    /// 长度大于等于
    LengthGte {
        field: String,
        min: usize,
    },
    /// 长度小于等于
    LengthLte {
        field: String,
        max: usize,
    },
    /// 正则匹配
    Matches {
        field: String,
        pattern: String,
    },
    /// 相等
    Equals {
        field: String,
        value: serde_json::Value,
    },
    /// 不相等
    NotEquals {
        field: String,
        value: serde_json::Value,
    },
    /// 大于
    GreaterThan {
        field: String,
        value: f64,
    },
    /// 小于
    LessThan {
        field: String,
        value: f64,
    },
    /// 存在
    Exists {
        field: String,
    },
    /// 不存在
    NotExists {
        field: String,
    },
    /// 组合规则：全部满足
    All {
        rules: Vec<Rule>,
    },
    /// 组合规则：任一满足
    Any {
        rules: Vec<Rule>,
    },
    /// 取反
    Not {
        rule: Box<Rule>,
    },
}

/// 验证结果
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// 是否通过
    pub passed: bool,
    /// 错误消息列表
    pub errors: Vec<String>,
}

impl ValidationResult {
    pub fn success() -> Self {
        Self {
            passed: true,
            errors: Vec::new(),
        }
    }

    pub fn failure(error: String) -> Self {
        Self {
            passed: false,
            errors: vec![error],
        }
    }

    pub fn merge(mut self, other: Self) -> Self {
        self.passed = self.passed && other.passed;
        self.errors.extend(other.errors);
        self
    }
}

/// 规则引擎
#[derive(Debug, Default)]
pub struct RuleEngine {
    /// 缓存的正则表达式
    regex_cache: HashMap<String, Regex>,
}

impl RuleEngine {
    pub fn new() -> Self {
        Self {
            regex_cache: HashMap::new(),
        }
    }

    /// 验证单个规则
    pub fn validate(&mut self, rule: &Rule, context: &HashMap<String, serde_json::Value>) -> Result<ValidationResult> {
        match rule {
            Rule::Contains { field, value, case_sensitive } => {
                self.validate_contains(context, field, value, *case_sensitive)
            }
            Rule::LengthGte { field, min } => {
                self.validate_length_gte(context, field, *min)
            }
            Rule::LengthLte { field, max } => {
                self.validate_length_lte(context, field, *max)
            }
            Rule::Matches { field, pattern } => {
                self.validate_matches(context, field, pattern)
            }
            Rule::Equals { field, value } => {
                self.validate_equals(context, field, value)
            }
            Rule::NotEquals { field, value } => {
                self.validate_not_equals(context, field, value)
            }
            Rule::GreaterThan { field, value } => {
                self.validate_greater_than(context, field, *value)
            }
            Rule::LessThan { field, value } => {
                self.validate_less_than(context, field, *value)
            }
            Rule::Exists { field } => {
                self.validate_exists(context, field)
            }
            Rule::NotExists { field } => {
                self.validate_not_exists(context, field)
            }
            Rule::All { rules } => {
                self.validate_all(rules, context)
            }
            Rule::Any { rules } => {
                self.validate_any(rules, context)
            }
            Rule::Not { rule } => {
                let result = self.validate(rule, context)?;
                if result.passed {
                    Ok(ValidationResult::failure("Condition should not be met".to_string()))
                } else {
                    Ok(ValidationResult::success())
                }
            }
        }
    }

    fn validate_contains(
        &self,
        context: &HashMap<String, serde_json::Value>,
        field: &str,
        value: &str,
        case_sensitive: bool,
    ) -> Result<ValidationResult> {
        match context.get(field) {
            Some(serde_json::Value::String(s)) => {
                let contains = if case_sensitive {
                    s.contains(value)
                } else {
                    s.to_lowercase().contains(&value.to_lowercase())
                };
                if contains {
                    Ok(ValidationResult::success())
                } else {
                    Ok(ValidationResult::failure(format!(
                        "Field '{}' does not contain '{}'",
                        field, value
                    )))
                }
            }
            Some(_) => Ok(ValidationResult::failure(format!(
                "Field '{}' is not a string",
                field
            ))),
            None => Ok(ValidationResult::failure(format!(
                "Field '{}' not found",
                field
            ))),
        }
    }

    fn validate_length_gte(
        &self,
        context: &HashMap<String, serde_json::Value>,
        field: &str,
        min: usize,
    ) -> Result<ValidationResult> {
        match context.get(field) {
            Some(serde_json::Value::String(s)) => {
                if s.len() >= min {
                    Ok(ValidationResult::success())
                } else {
                    Ok(ValidationResult::failure(format!(
                        "Field '{}' length {} is less than {}",
                        field,
                        s.len(),
                        min
                    )))
                }
            }
            Some(serde_json::Value::Array(arr)) => {
                if arr.len() >= min {
                    Ok(ValidationResult::success())
                } else {
                    Ok(ValidationResult::failure(format!(
                        "Field '{}' array length {} is less than {}",
                        field,
                        arr.len(),
                        min
                    )))
                }
            }
            Some(_) => Ok(ValidationResult::failure(format!(
                "Field '{}' is not a string or array",
                field
            ))),
            None => Ok(ValidationResult::failure(format!(
                "Field '{}' not found",
                field
            ))),
        }
    }

    fn validate_length_lte(
        &self,
        context: &HashMap<String, serde_json::Value>,
        field: &str,
        max: usize,
    ) -> Result<ValidationResult> {
        match context.get(field) {
            Some(serde_json::Value::String(s)) => {
                if s.len() <= max {
                    Ok(ValidationResult::success())
                } else {
                    Ok(ValidationResult::failure(format!(
                        "Field '{}' length {} is greater than {}",
                        field,
                        s.len(),
                        max
                    )))
                }
            }
            Some(serde_json::Value::Array(arr)) => {
                if arr.len() <= max {
                    Ok(ValidationResult::success())
                } else {
                    Ok(ValidationResult::failure(format!(
                        "Field '{}' array length {} is greater than {}",
                        field,
                        arr.len(),
                        max
                    )))
                }
            }
            Some(_) => Ok(ValidationResult::failure(format!(
                "Field '{}' is not a string or array",
                field
            ))),
            None => Ok(ValidationResult::failure(format!(
                "Field '{}' not found",
                field
            ))),
        }
    }

    fn validate_matches(
        &mut self,
        context: &HashMap<String, serde_json::Value>,
        field: &str,
        pattern: &str,
    ) -> Result<ValidationResult> {
        let regex = self.regex_cache
            .entry(pattern.to_string())
            .or_insert_with(|| {
                Regex::new(pattern).unwrap_or_else(|_| Regex::new("^(?:)$").unwrap())
            });

        match context.get(field) {
            Some(serde_json::Value::String(s)) => {
                if regex.is_match(s) {
                    Ok(ValidationResult::success())
                } else {
                    Ok(ValidationResult::failure(format!(
                        "Field '{}' does not match pattern '{}'",
                        field, pattern
                    )))
                }
            }
            Some(_) => Ok(ValidationResult::failure(format!(
                "Field '{}' is not a string",
                field
            ))),
            None => Ok(ValidationResult::failure(format!(
                "Field '{}' not found",
                field
            ))),
        }
    }

    fn validate_equals(
        &self,
        context: &HashMap<String, serde_json::Value>,
        field: &str,
        value: &serde_json::Value,
    ) -> Result<ValidationResult> {
        match context.get(field) {
            Some(v) => {
                if v == value {
                    Ok(ValidationResult::success())
                } else {
                    Ok(ValidationResult::failure(format!(
                        "Field '{}' value {:?} does not equal {:?}",
                        field, v, value
                    )))
                }
            }
            None => Ok(ValidationResult::failure(format!(
                "Field '{}' not found",
                field
            ))),
        }
    }

    fn validate_not_equals(
        &self,
        context: &HashMap<String, serde_json::Value>,
        field: &str,
        value: &serde_json::Value,
    ) -> Result<ValidationResult> {
        match context.get(field) {
            Some(v) => {
                if v != value {
                    Ok(ValidationResult::success())
                } else {
                    Ok(ValidationResult::failure(format!(
                        "Field '{}' value {:?} equals {:?}",
                        field, v, value
                    )))
                }
            }
            None => Ok(ValidationResult::success()),
        }
    }

    fn validate_greater_than(
        &self,
        context: &HashMap<String, serde_json::Value>,
        field: &str,
        value: f64,
    ) -> Result<ValidationResult> {
        match context.get(field) {
            Some(serde_json::Value::Number(n)) => {
                if let Some(f) = n.as_f64() {
                    if f > value {
                        Ok(ValidationResult::success())
                    } else {
                        Ok(ValidationResult::failure(format!(
                            "Field '{}' value {} is not greater than {}",
                            field, f, value
                        )))
                    }
                } else {
                    Ok(ValidationResult::failure(format!(
                        "Field '{}' is not a valid number",
                        field
                    )))
                }
            }
            Some(_) => Ok(ValidationResult::failure(format!(
                "Field '{}' is not a number",
                field
            ))),
            None => Ok(ValidationResult::failure(format!(
                "Field '{}' not found",
                field
            ))),
        }
    }

    fn validate_less_than(
        &self,
        context: &HashMap<String, serde_json::Value>,
        field: &str,
        value: f64,
    ) -> Result<ValidationResult> {
        match context.get(field) {
            Some(serde_json::Value::Number(n)) => {
                if let Some(f) = n.as_f64() {
                    if f < value {
                        Ok(ValidationResult::success())
                    } else {
                        Ok(ValidationResult::failure(format!(
                            "Field '{}' value {} is not less than {}",
                            field, f, value
                        )))
                    }
                } else {
                    Ok(ValidationResult::failure(format!(
                        "Field '{}' is not a valid number",
                        field
                    )))
                }
            }
            Some(_) => Ok(ValidationResult::failure(format!(
                "Field '{}' is not a number",
                field
            ))),
            None => Ok(ValidationResult::failure(format!(
                "Field '{}' not found",
                field
            ))),
        }
    }

    fn validate_exists(
        &self,
        context: &HashMap<String, serde_json::Value>,
        field: &str,
    ) -> Result<ValidationResult> {
        if context.contains_key(field) {
            Ok(ValidationResult::success())
        } else {
            Ok(ValidationResult::failure(format!(
                "Field '{}' not found",
                field
            )))
        }
    }

    fn validate_not_exists(
        &self,
        context: &HashMap<String, serde_json::Value>,
        field: &str,
    ) -> Result<ValidationResult> {
        if !context.contains_key(field) {
            Ok(ValidationResult::success())
        } else {
            Ok(ValidationResult::failure(format!(
                "Field '{}' exists",
                field
            )))
        }
    }

    fn validate_all(
        &mut self,
        rules: &[Rule],
        context: &HashMap<String, serde_json::Value>,
    ) -> Result<ValidationResult> {
        let mut result = ValidationResult::success();
        for rule in rules {
            result = result.merge(self.validate(rule, context)?);
        }
        Ok(result)
    }

    fn validate_any(
        &mut self,
        rules: &[Rule],
        context: &HashMap<String, serde_json::Value>,
    ) -> Result<ValidationResult> {
        let mut errors = Vec::new();
        for rule in rules {
            let result = self.validate(rule, context)?;
            if result.passed {
                return Ok(ValidationResult::success());
            }
            errors.extend(result.errors);
        }
        Ok(ValidationResult::failure(format!(
            "None of the conditions met: {}",
            errors.join("; ")
        )))
    }
}

/// 表达式求值（简单比较和逻辑运算）
pub fn evaluate_expression(expr: &str, context: &HashMap<String, serde_json::Value>) -> Result<bool> {
    let expr = expr.trim();

    // 处理 AND 逻辑
    if expr.contains(" && ") {
        let parts: Vec<&str> = expr.split(" && ").collect();
        for part in parts {
            if !evaluate_expression(part, context)? {
                return Ok(false);
            }
        }
        return Ok(true);
    }

    // 处理 OR 逻辑
    if expr.contains(" || ") {
        let parts: Vec<&str> = expr.split(" || ").collect();
        for part in parts {
            if evaluate_expression(part, context)? {
                return Ok(true);
            }
        }
        return Ok(false);
    }

    // 处理比较运算
    // 相等检查
    if let Some(eq_pos) = expr.find("==") {
        let left = expr[..eq_pos].trim();
        let right = expr[eq_pos + 2..].trim();
        return evaluate_comparison(left, right, context, true);
    }

    // 不等检查
    if let Some(ne_pos) = expr.find("!=") {
        let left = expr[..ne_pos].trim();
        let right = expr[ne_pos + 2..].trim();
        return evaluate_comparison(left, right, context, false);
    }

    // 大于等于
    if let Some(ge_pos) = expr.find(">=") {
        let left = expr[..ge_pos].trim();
        let right = expr[ge_pos + 2..].trim();
        return evaluate_numeric_comparison(left, right, context, ">=");
    }

    // 小于等于
    if let Some(le_pos) = expr.find("<=") {
        let left = expr[..le_pos].trim();
        let right = expr[le_pos + 2..].trim();
        return evaluate_numeric_comparison(left, right, context, "<=");
    }

    // 大于
    if let Some(gt_pos) = expr.find('>') {
        let left = expr[..gt_pos].trim();
        let right = expr[gt_pos + 1..].trim();
        return evaluate_numeric_comparison(left, right, context, ">");
    }

    // 小于
    if let Some(lt_pos) = expr.find('<') {
        let left = expr[..lt_pos].trim();
        let right = expr[lt_pos + 1..].trim();
        return evaluate_numeric_comparison(left, right, context, "<");
    }

    // 布尔值检查
    match expr {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => {
            // 检查变量是否存在且为真
            if let Some(value) = context.get(expr) {
                Ok(value.as_bool().unwrap_or(false))
            } else {
                Ok(false)
            }
        }
    }
}

fn evaluate_comparison(
    left: &str,
    right: &str,
    context: &HashMap<String, serde_json::Value>,
    equals: bool,
) -> Result<bool> {
    let left_val = resolve_value(left, context)?;
    let right_val = resolve_value(right, context)?;

    let result = left_val == right_val;
    Ok(if equals { result } else { !result })
}

fn evaluate_numeric_comparison(
    left: &str,
    right: &str,
    context: &HashMap<String, serde_json::Value>,
    op: &str,
) -> Result<bool> {
    let left_val = resolve_numeric(left, context)
        .with_context(|| format!("Failed to resolve left operand: {}", left))?;
    let right_val = resolve_numeric(right, context)
        .with_context(|| format!("Failed to resolve right operand: {}", right))?;

    let result = match op {
        ">" => left_val > right_val,
        "<" => left_val < right_val,
        ">=" => left_val >= right_val,
        "<=" => left_val <= right_val,
        _ => false,
    };

    Ok(result)
}

fn resolve_value(expr: &str, context: &HashMap<String, serde_json::Value>) -> Result<serde_json::Value> {
    // 字符串字面量
    if expr.starts_with('"') && expr.ends_with('"') {
        return Ok(serde_json::Value::String(expr[1..expr.len()-1].to_string()));
    }

    // 数字字面量
    if let Ok(n) = expr.parse::<i64>() {
        return Ok(serde_json::Value::Number(n.into()));
    }
    if let Ok(n) = expr.parse::<f64>()
        && let Some(num) = serde_json::Number::from_f64(n) {
            return Ok(serde_json::Value::Number(num));
        }

    // 布尔字面量
    if expr == "true" {
        return Ok(serde_json::Value::Bool(true));
    }
    if expr == "false" {
        return Ok(serde_json::Value::Bool(false));
    }

    // 空值
    if expr == "null" {
        return Ok(serde_json::Value::Null);
    }

    // 变量引用
    if let Some(value) = context.get(expr) {
        return Ok(value.clone());
    }

    anyhow::bail!("Unknown value: {}", expr)
}

fn resolve_numeric(expr: &str, context: &HashMap<String, serde_json::Value>) -> Result<f64> {
    // 数字字面量
    if let Ok(n) = expr.parse::<f64>() {
        return Ok(n);
    }

    // 变量引用
    if let Some(value) = context.get(expr)
        && let Some(n) = value.as_f64() {
            return Ok(n);
        }

    anyhow::bail!("Not a numeric value: {}", expr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_rule_contains() {
        let mut engine = RuleEngine::new();
        let mut context = HashMap::new();
        context.insert("text".to_string(), json!("Hello, World!"));

        let rule = Rule::Contains {
            field: "text".to_string(),
            value: "World".to_string(),
            case_sensitive: true,
        };

        let result = engine.validate(&rule, &context).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_rule_length_gte() {
        let mut engine = RuleEngine::new();
        let mut context = HashMap::new();
        context.insert("name".to_string(), json!("Alice"));

        let rule = Rule::LengthGte {
            field: "name".to_string(),
            min: 3,
        };

        let result = engine.validate(&rule, &context).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_rule_matches() {
        let mut engine = RuleEngine::new();
        let mut context = HashMap::new();
        context.insert("email".to_string(), json!("test@example.com"));

        let rule = Rule::Matches {
            field: "email".to_string(),
            pattern: r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$".to_string(),
        };

        let result = engine.validate(&rule, &context).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_rule_all() {
        let mut engine = RuleEngine::new();
        let mut context = HashMap::new();
        context.insert("name".to_string(), json!("Alice"));
        context.insert("age".to_string(), json!(25));

        let rule = Rule::All {
            rules: vec![
                Rule::LengthGte { field: "name".to_string(), min: 3 },
                Rule::GreaterThan { field: "age".to_string(), value: 18.0 },
            ],
        };

        let result = engine.validate(&rule, &context).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_evaluate_expression() {
        let mut context = HashMap::new();
        context.insert("count".to_string(), json!(10));
        context.insert("enabled".to_string(), json!(true));

        assert!(evaluate_expression("count == 10", &context).unwrap());
        assert!(evaluate_expression("count > 5", &context).unwrap());
        assert!(evaluate_expression("count < 20 && enabled == true", &context).unwrap());
        assert!(evaluate_expression("count < 5 || enabled == true", &context).unwrap());
    }
}