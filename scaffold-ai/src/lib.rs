use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: String,
    pub model: String,
    pub usage: Option<LlmUsage>,
}

#[derive(Debug, Clone)]
pub struct LlmUsage {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
}

#[derive(Debug)]
pub enum LlmError {
    ProviderError(String),
    MalformedOutput(String),
    SchemaValidation(String),
    Timeout,
}

impl std::fmt::Display for LlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub type LlmResult<T> = std::result::Result<T, LlmError>;

pub trait LlmProvider: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn send(&self, prompt: &str) -> std::pin::Pin<Box<dyn std::future::Future<Output = LlmResult<LlmResponse>> + Send>>;
}

#[derive(Debug, Clone, Serialize)]
pub struct ExtractPrompt {
    pub graph_description: String,
    pub task_description: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExtractRequest {
    pub prompt: String,
    pub schema: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExtractProposal {
    pub tasks: Vec<TaskProposal>,
    pub plan_order: Vec<String>,
    pub cycle: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskProposal {
    pub node_id: String,
    pub title: String,
    pub body: String,
    pub effort: Option<f64>,
    pub source_node: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MockProvider;

impl MockProvider {
    pub fn new() -> Self {
        MockProvider
    }
}

impl LlmProvider for MockProvider {
    fn name(&self) -> &str {
        "mock"
    }

    fn send(&self, _prompt: &str) -> std::pin::Pin<Box<dyn std::future::Future<Output = LlmResult<LlmResponse>> + Send>> {
        Box::pin(async move {
            Ok(LlmResponse {
                content: r#"{"tasks":[{"node_id":"n1","title":"Task 1","body":"Do something","effort":2.0,"source_node":null}],"plan_order":["n1"],"cycle":null}"#.to_string(),
                model: "mock".to_string(),
                usage: Some(LlmUsage {
                    prompt_tokens: 10,
                    completion_tokens: 20,
                    total_tokens: 30,
                }),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_provider() {
        let provider = MockProvider::new();
        assert_eq!(provider.name(), "mock");
    }

    #[test]
    fn test_extract_proposal_deserialization() {
        let json = r#"{"tasks":[{"node_id":"n1","title":"Task 1","body":"Do something","effort":2.0,"source_node":null}],"plan_order":["n1"],"cycle":null}"#;
        let proposal: ExtractProposal = serde_json::from_str(json).unwrap();
        assert_eq!(proposal.tasks.len(), 1);
        assert_eq!(proposal.tasks[0].title, "Task 1");
        assert_eq!(proposal.plan_order, vec!["n1"]);
        assert!(proposal.cycle.is_none());
    }
}