//! Time and date tools.
//!
//! Provides tools for working with dates, times, timestamps,
//! and time calculations.

use crate::tools::{Tool, ToolContext};
use crate::Result;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Local, TimeZone, Utc};
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use std::time::Instant;
use tracing::debug;

/// Tool for getting current time.
pub struct NowTool;

impl NowTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NowTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for NowTool {
    fn name(&self) -> &str {
        "now"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "now".to_string(),
            description: "Get the current date and time.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "timezone": {
                        "type": "string",
                        "default": "local",
                        "description": "Timezone: 'local', 'utc', or offset like '+05:30'"
                    },
                    "format": {
                        "type": "string",
                        "description": "Custom format string (strftime-style)"
                    }
                }
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let timezone = args
            .get("timezone")
            .and_then(|v| v.as_str())
            .unwrap_or("local");

        let format = args.get("format").and_then(|v| v.as_str());

        let now_utc = Utc::now();
        let now_local = Local::now();

        let formatted = match timezone {
            "utc" => {
                if let Some(fmt) = format {
                    now_utc.format(fmt).to_string()
                } else {
                    now_utc.to_rfc3339()
                }
            }
            "local" => {
                if let Some(fmt) = format {
                    now_local.format(fmt).to_string()
                } else {
                    now_local.to_rfc3339()
                }
            }
            _ => {
                // Try to parse as offset
                if let Some(fmt) = format {
                    now_utc.format(fmt).to_string()
                } else {
                    now_utc.to_rfc3339()
                }
            }
        };

        let duration = start.elapsed();

        debug!("Now: {}", formatted);

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "datetime": formatted,
                "timestamp": now_utc.timestamp(),
                "timestamp_millis": now_utc.timestamp_millis(),
                "iso8601": now_utc.to_rfc3339(),
                "date": now_utc.format("%Y-%m-%d").to_string(),
                "time": now_utc.format("%H:%M:%S").to_string(),
                "timezone": timezone,
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Tool for parsing and formatting dates.
pub struct DateParseTool;

impl DateParseTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DateParseTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for DateParseTool {
    fn name(&self) -> &str {
        "date_parse"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "date_parse".to_string(),
            description: "Parse a date string and convert to different formats.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "Date string to parse"
                    },
                    "input_format": {
                        "type": "string",
                        "description": "Expected input format (optional, will try common formats)"
                    },
                    "output_format": {
                        "type": "string",
                        "description": "Desired output format (strftime-style)"
                    }
                },
                "required": ["input"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let input = args
            .get("input")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AgentError::tool_execution("input is required"))?;

        let output_format = args.get("output_format").and_then(|v| v.as_str());

        // Try to parse the date using various common formats
        let parsed: Option<DateTime<Utc>> = None
            .or_else(|| DateTime::parse_from_rfc3339(input).ok().map(|d| d.with_timezone(&Utc)))
            .or_else(|| DateTime::parse_from_rfc2822(input).ok().map(|d| d.with_timezone(&Utc)))
            .or_else(|| {
                chrono::NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M:%S")
                    .ok()
                    .map(|d| Utc.from_utc_datetime(&d))
            })
            .or_else(|| {
                chrono::NaiveDateTime::parse_from_str(input, "%Y-%m-%dT%H:%M:%S")
                    .ok()
                    .map(|d| Utc.from_utc_datetime(&d))
            })
            .or_else(|| {
                chrono::NaiveDate::parse_from_str(input, "%Y-%m-%d")
                    .ok()
                    .and_then(|d| d.and_hms_opt(0, 0, 0))
                    .map(|d| Utc.from_utc_datetime(&d))
            })
            .or_else(|| {
                chrono::NaiveDate::parse_from_str(input, "%m/%d/%Y")
                    .ok()
                    .and_then(|d| d.and_hms_opt(0, 0, 0))
                    .map(|d| Utc.from_utc_datetime(&d))
            })
            .or_else(|| {
                chrono::NaiveDate::parse_from_str(input, "%d/%m/%Y")
                    .ok()
                    .and_then(|d| d.and_hms_opt(0, 0, 0))
                    .map(|d| Utc.from_utc_datetime(&d))
            })
            .or_else(|| {
                // Try parsing as timestamp
                input.parse::<i64>().ok().and_then(|ts| {
                    Utc.timestamp_opt(ts, 0).single()
                })
            })
            .or_else(|| {
                // Try parsing as timestamp millis
                input.parse::<i64>().ok().and_then(|ts| {
                    Utc.timestamp_millis_opt(ts).single()
                })
            });

        let parsed = match parsed {
            Some(dt) => dt,
            None => {
                return Ok(ToolResult::error(
                    tool_use_id,
                    format!("Could not parse date: '{}'", input),
                ));
            }
        };

        let formatted = if let Some(fmt) = output_format {
            parsed.format(fmt).to_string()
        } else {
            parsed.to_rfc3339()
        };

        let duration = start.elapsed();

        debug!("Date parsed: {} -> {}", input, formatted);

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "formatted": formatted,
                "iso8601": parsed.to_rfc3339(),
                "timestamp": parsed.timestamp(),
                "timestamp_millis": parsed.timestamp_millis(),
                "year": parsed.format("%Y").to_string(),
                "month": parsed.format("%m").to_string(),
                "day": parsed.format("%d").to_string(),
                "hour": parsed.format("%H").to_string(),
                "minute": parsed.format("%M").to_string(),
                "second": parsed.format("%S").to_string(),
                "weekday": parsed.format("%A").to_string(),
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Tool for date calculations.
pub struct DateCalcTool;

impl DateCalcTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DateCalcTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for DateCalcTool {
    fn name(&self) -> &str {
        "date_calc"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "date_calc".to_string(),
            description: "Perform date calculations (add/subtract time, calculate difference)."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "date": {
                        "type": "string",
                        "description": "Base date (ISO 8601 format, or 'now')"
                    },
                    "operation": {
                        "type": "string",
                        "enum": ["add", "subtract", "diff"],
                        "description": "Operation to perform"
                    },
                    "days": {
                        "type": "integer",
                        "description": "Number of days"
                    },
                    "hours": {
                        "type": "integer",
                        "description": "Number of hours"
                    },
                    "minutes": {
                        "type": "integer",
                        "description": "Number of minutes"
                    },
                    "seconds": {
                        "type": "integer",
                        "description": "Number of seconds"
                    },
                    "other_date": {
                        "type": "string",
                        "description": "Second date for diff operation"
                    }
                },
                "required": ["date", "operation"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let date_str = args
            .get("date")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AgentError::tool_execution("date is required"))?;

        let operation = args
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AgentError::tool_execution("operation is required"))?;

        // Parse the base date
        let base_date: DateTime<Utc> = if date_str == "now" {
            Utc::now()
        } else {
            DateTime::parse_from_rfc3339(date_str)
                .map(|d| d.with_timezone(&Utc))
                .or_else(|_| {
                    chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%d %H:%M:%S")
                        .map(|d| Utc.from_utc_datetime(&d))
                })
                .or_else(|_| {
                    chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                        .ok()
                        .and_then(|d| d.and_hms_opt(0, 0, 0))
                        .map(|d| Utc.from_utc_datetime(&d))
                        .ok_or_else(|| crate::error::AgentError::tool_execution(format!("Invalid date: {}", date_str)))
                })?
        };

        match operation {
            "add" | "subtract" => {
                let days = args.get("days").and_then(|v| v.as_i64()).unwrap_or(0);
                let hours = args.get("hours").and_then(|v| v.as_i64()).unwrap_or(0);
                let minutes = args.get("minutes").and_then(|v| v.as_i64()).unwrap_or(0);
                let seconds = args.get("seconds").and_then(|v| v.as_i64()).unwrap_or(0);

                let duration = Duration::days(days)
                    + Duration::hours(hours)
                    + Duration::minutes(minutes)
                    + Duration::seconds(seconds);

                let result = if operation == "add" {
                    base_date + duration
                } else {
                    base_date - duration
                };

                let exec_duration = start.elapsed();

                debug!("Date calc: {} {} = {}", date_str, operation, result.to_rfc3339());

                Ok(ToolResult::success(
                    tool_use_id,
                    serde_json::json!({
                        "result": result.to_rfc3339(),
                        "timestamp": result.timestamp(),
                        "operation": operation,
                        "original": base_date.to_rfc3339(),
                    }),
                )
                .with_duration(exec_duration))
            }
            "diff" => {
                let other_str = args
                    .get("other_date")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| crate::error::AgentError::tool_execution("other_date is required for diff"))?;

                let other_date: DateTime<Utc> = if other_str == "now" {
                    Utc::now()
                } else {
                    DateTime::parse_from_rfc3339(other_str)
                        .map(|d| d.with_timezone(&Utc))
                        .or_else(|_| {
                            chrono::NaiveDateTime::parse_from_str(other_str, "%Y-%m-%d %H:%M:%S")
                                .map(|d| Utc.from_utc_datetime(&d))
                        })
                        .map_err(|_| crate::error::AgentError::tool_execution(format!("Invalid other_date: {}", other_str)))?
                };

                let diff = other_date.signed_duration_since(base_date);
                let total_seconds = diff.num_seconds();
                let days = diff.num_days();
                let hours = (total_seconds % 86400) / 3600;
                let minutes = (total_seconds % 3600) / 60;
                let seconds = total_seconds % 60;

                let exec_duration = start.elapsed();

                debug!("Date diff: {} days", days);

                Ok(ToolResult::success(
                    tool_use_id,
                    serde_json::json!({
                        "days": days,
                        "hours": hours,
                        "minutes": minutes,
                        "seconds": seconds,
                        "total_seconds": total_seconds,
                        "total_hours": diff.num_hours(),
                        "total_minutes": diff.num_minutes(),
                        "from": base_date.to_rfc3339(),
                        "to": other_date.to_rfc3339(),
                    }),
                )
                .with_duration(exec_duration))
            }
            _ => Ok(ToolResult::error(
                tool_use_id,
                format!("Unknown operation: {}", operation),
            )),
        }
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_now_tool_creation() {
        let tool = NowTool::new();
        assert_eq!(tool.name(), "now");
    }

    #[test]
    fn test_date_parse_tool_creation() {
        let tool = DateParseTool::new();
        assert_eq!(tool.name(), "date_parse");
    }

    #[test]
    fn test_date_calc_tool_creation() {
        let tool = DateCalcTool::new();
        assert_eq!(tool.name(), "date_calc");
    }

    #[tokio::test]
    async fn test_now_execute() {
        let tool = NowTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute("test_id", serde_json::json!({}), &ctx)
            .await
            .unwrap();

        assert!(!result.is_error);
        assert!(result.output.get("datetime").is_some());
        assert!(result.output.get("timestamp").is_some());
    }

    #[tokio::test]
    async fn test_now_with_format() {
        let tool = NowTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "format": "%Y-%m-%d"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let datetime = result.output.get("datetime").and_then(|v| v.as_str()).unwrap();
        // Should be in YYYY-MM-DD format
        assert!(datetime.len() == 10);
        assert!(datetime.contains('-'));
    }

    #[tokio::test]
    async fn test_date_parse() {
        let tool = DateParseTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "input": "2024-06-15"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("year").and_then(|v| v.as_str()),
            Some("2024")
        );
        assert_eq!(
            result.output.get("month").and_then(|v| v.as_str()),
            Some("06")
        );
        assert_eq!(
            result.output.get("day").and_then(|v| v.as_str()),
            Some("15")
        );
    }

    #[tokio::test]
    async fn test_date_parse_timestamp() {
        let tool = DateParseTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "input": "1718409600"  // 2024-06-15 00:00:00 UTC
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert!(result.output.get("iso8601").is_some());
    }

    #[tokio::test]
    async fn test_date_calc_add() {
        let tool = DateCalcTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "date": "2024-06-15T00:00:00Z",
                    "operation": "add",
                    "days": 10
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let result_str = result.output.get("result").and_then(|v| v.as_str()).unwrap();
        assert!(result_str.contains("2024-06-25"));
    }

    #[tokio::test]
    async fn test_date_calc_diff() {
        let tool = DateCalcTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "date": "2024-06-01T00:00:00Z",
                    "operation": "diff",
                    "other_date": "2024-06-15T00:00:00Z"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("days").and_then(|v| v.as_i64()),
            Some(14)
        );
    }
}
