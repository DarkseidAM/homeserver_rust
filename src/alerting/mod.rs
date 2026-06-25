// Threshold alerting: evaluate configured rules against each snapshot and emit fire/resolve
// events. Pure state machine (clock injected) + a notifier that logs and optionally POSTs a webhook.

mod metrics;
mod notify;

pub use metrics::{compare, extract_metric};
pub use notify::Notifier;

use crate::config::AlertRule;
use crate::models::FullSystemSnapshot;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertState {
    Firing,
    Resolved,
}

/// A state transition for one rule, produced by [`AlertEngine::evaluate`].
#[derive(Debug, Clone)]
pub struct AlertEvent {
    pub rule_name: String,
    pub metric: String,
    pub op: String,
    pub value: f64,
    pub threshold: f64,
    pub state: AlertState,
}

#[derive(Default)]
struct RuleState {
    breached_since: Option<Instant>,
    firing: bool,
    last_fired: Option<Instant>,
}

/// Evaluates alert rules against snapshots, tracking sustain/cooldown timing per rule.
pub struct AlertEngine {
    rules: Vec<AlertRule>,
    states: Vec<RuleState>,
}

impl AlertEngine {
    pub fn new(rules: Vec<AlertRule>) -> Self {
        let states = rules.iter().map(|_| RuleState::default()).collect();
        Self { rules, states }
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Evaluate all rules at instant `now`. Emits a `Firing` event when a rule has been breached
    /// for at least `duration_secs` and the `cooldown_secs` debounce has elapsed, and a `Resolved`
    /// event when a firing rule recovers.
    pub fn evaluate(&mut self, snapshot: &FullSystemSnapshot, now: Instant) -> Vec<AlertEvent> {
        let mut events = Vec::new();
        for (rule, st) in self.rules.iter().zip(self.states.iter_mut()) {
            let Some(value) = extract_metric(&rule.metric, snapshot) else {
                continue;
            };
            if compare(value, &rule.op, rule.threshold) {
                let since = *st.breached_since.get_or_insert(now);
                let sustained =
                    now.duration_since(since) >= Duration::from_secs(rule.duration_secs);
                let cooled = st.last_fired.is_none_or(|t| {
                    now.duration_since(t) >= Duration::from_secs(rule.cooldown_secs)
                });
                if sustained && !st.firing && cooled {
                    st.firing = true;
                    st.last_fired = Some(now);
                    events.push(event(rule, value, AlertState::Firing));
                }
            } else {
                if st.firing {
                    events.push(event(rule, value, AlertState::Resolved));
                }
                st.firing = false;
                st.breached_since = None;
            }
        }
        events
    }
}

fn event(rule: &AlertRule, value: f64, state: AlertState) -> AlertEvent {
    AlertEvent {
        rule_name: rule.name.clone(),
        metric: rule.metric.clone(),
        op: rule.op.clone(),
        value,
        threshold: rule.threshold,
        state,
    }
}
