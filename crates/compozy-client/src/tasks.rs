use crate::{Client, Error, request::RouteKind, sessions::encode_segment};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskVerb {
    Approve,
    Reject,
    Retry,
}

impl Client {
    pub async fn task_verb(
        &self,
        workspace: &str,
        task_id: &str,
        verb: TaskVerb,
    ) -> Result<(), Error> {
        let task_id = encode_segment(task_id);
        let path = match verb {
            TaskVerb::Approve => format!(
                "/api/workspaces/{}/tasks/{task_id}/approve",
                encode_segment(workspace)
            ),
            TaskVerb::Reject => format!(
                "/api/workspaces/{}/tasks/{task_id}/reject",
                encode_segment(workspace)
            ),
            TaskVerb::Retry => format!("/api/tasks/{task_id}/runs"),
        };
        self.post_empty(&path, "task verb response", RouteKind::Scoped)
            .await
    }
}
