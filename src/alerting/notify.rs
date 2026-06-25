// Alert delivery: always log via tracing; optionally POST a JSON payload to a webhook.

use super::{AlertEvent, AlertState};

/// Logs every alert event and, when a webhook URL is configured, POSTs it as JSON.
/// Cloneable (reqwest::Client is internally reference-counted) so it can be moved into tasks.
#[derive(Clone)]
pub struct Notifier {
    client: Option<reqwest::Client>,
    webhook_url: Option<String>,
}

impl Notifier {
    pub fn new(webhook_url: Option<String>) -> Self {
        let client = webhook_url.as_ref().map(|_| reqwest::Client::new());
        Self {
            client,
            webhook_url,
        }
    }

    pub async fn notify(&self, ev: &AlertEvent) {
        match ev.state {
            AlertState::Firing => tracing::warn!(
                rule = %ev.rule_name, metric = %ev.metric, op = %ev.op,
                value = ev.value, threshold = ev.threshold, "alert firing"
            ),
            AlertState::Resolved => tracing::info!(
                rule = %ev.rule_name, metric = %ev.metric, "alert resolved"
            ),
        }

        if let (Some(client), Some(url)) = (&self.client, &self.webhook_url) {
            let state = match ev.state {
                AlertState::Firing => "firing",
                AlertState::Resolved => "resolved",
            };
            let payload = serde_json::json!({
                "rule": ev.rule_name,
                "metric": ev.metric,
                "op": ev.op,
                "value": ev.value,
                "threshold": ev.threshold,
                "state": state,
            });
            if let Err(e) = client.post(url).json(&payload).send().await {
                tracing::warn!(error = %e, rule = %ev.rule_name, "alert webhook POST failed");
            }
        }
    }
}
