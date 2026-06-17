//! Tool for resolving `codewhale://workroom/...` links to scoped context.
//!
//! See [RFC 3209](../../docs/rfcs/3209-workrooms.md).

use async_trait::async_trait;
use serde_json::{Value, json};

use super::spec::{
    ApprovalRequirement, ToolCapability, ToolContext, ToolError, ToolResult, ToolSpec, required_str,
};

/// Resolves a `codewhale://workroom/...` link to thread metadata, external refs,
/// and recent events without replaying the full transcript.
///
/// This is useful when a link appears in a chat message (TUI, bridge, mobile)
/// and the model needs to understand what it points to before continuing.
pub struct ResolveWorkroomLinkTool;

#[async_trait]
impl ToolSpec for ResolveWorkroomLinkTool {
    fn name(&self) -> &'static str {
        "resolve_workroom_link"
    }

    fn description(&self) -> &'static str {
        "Resolve a codewhale://workroom/... link to thread metadata, \
         external references (GitHub issues/PRs/commits), and recent event \
         summaries without replaying the full transcript."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "A codewhale://workroom/... URL to resolve"
                }
            },
            "required": ["url"]
        })
    }

    fn capabilities(&self) -> Vec<ToolCapability> {
        vec![ToolCapability::ReadOnly]
    }

    fn approval_requirement(&self) -> ApprovalRequirement {
        ApprovalRequirement::Auto
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolResult, ToolError> {
        let url = required_str(&input, "url")?;

        // Parse the link using the protocol crate's parser.
        match codewhale_protocol::workroom::WorkroomLink::parse(url) {
            Some(link) => {
                let result = json!({
                    "workroom_id": link.workroom_id.to_string(),
                    "thread_id": link.thread_id,
                    "event_id": link.event_id,
                    "resolved": false,
                    "note": "Link parsed successfully. Full resolution (thread metadata, \
                             external refs, recent events) requires accessing the Runtime \
                             API. This stub will be replaced in Phase 2 of the Workrooms EPIC \
                             (issue #3209)."
                });
                Ok(ToolResult::success(result.to_string()))
            }
            None => Ok(ToolResult::success(
                json!({"error": "invalid workroom link format", "url": url}).to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn ctx() -> ToolContext {
        ToolContext::new(Path::new("."))
    }

    #[tokio::test]
    async fn parses_valid_workroom_link() {
        let result = ResolveWorkroomLinkTool
            .execute(
                json!({"url": "codewhale://workroom/wr_abc123/thread/thr_xyz"}),
                &ctx(),
            )
            .await
            .expect("should parse valid link");
        assert!(result.content.contains("wr_abc123"), "{result:?}");
        assert!(result.content.contains("thr_xyz"), "{result:?}");
    }

    #[tokio::test]
    async fn rejects_invalid_url_format() {
        let result = ResolveWorkroomLinkTool
            .execute(json!({"url": "not-a-workroom-link"}), &ctx())
            .await
            .expect("should succeed with error in content");
        assert!(
            result.content.contains("invalid workroom link format"),
            "{result:?}"
        );
    }

    #[tokio::test]
    async fn rejects_missing_url_field() {
        let err = ResolveWorkroomLinkTool
            .execute(json!({}), &ctx())
            .await
            .unwrap_err();
        assert!(err.to_string().to_lowercase().contains("url"), "{err}");
    }
}
