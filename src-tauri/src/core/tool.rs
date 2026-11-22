use std::sync::Arc;

/// Contract implemented by every tool hosted inside XTools (mirrors macOS `XToolModule`).
pub trait XTool: Send + Sync {
  fn id(&self) -> &'static str;
  fn activate(&self);
  fn shutdown(&self);
}

/// Owns all registered tools so background monitors start once at app launch
/// and shut down cleanly on exit.
pub struct ToolRegistry {
  tools: Vec<Arc<dyn XTool>>,
}

impl ToolRegistry {
  pub fn new(tools: Vec<Arc<dyn XTool>>) -> Self {
    Self { tools }
  }

  pub fn activate_all(&self) {
    for tool in &self.tools {
      tool.activate();
    }
  }

  pub fn shutdown_all(&self) {
    for tool in &self.tools {
      tool.shutdown();
    }
  }
}
