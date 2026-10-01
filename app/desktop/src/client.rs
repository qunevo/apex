//! Read-only access to the control platform: snapshots through the HTTP API and
//! change notifications through its server-sent event stream.
use apex_control::{PlanResult, ResultSummary, Run, Scenario, ScenarioId};
use futures_channel::mpsc::UnboundedSender;
use serde::de::DeserializeOwned;
use std::{
    io::{BufRead, BufReader},
    time::Duration,
};

#[derive(Clone)]
pub struct Client {
    base: String,
    token: String,
    agent: ureq::Agent,
}

/// Everything the window displays, fetched together after each change.
#[derive(Clone, Default)]
pub struct Snapshot {
    pub scenarios: Vec<Scenario>,
    pub detail: Option<Detail>,
}

#[derive(Clone)]
pub struct Detail {
    pub scenario: Scenario,
    pub runs: Vec<Run>,
    pub results: Vec<ResultSummary>,
    /// The published result, otherwise the newest proposed or approved one.
    pub plan: Option<PlanResult>,
}

/// Connection state shown in the status bar.
#[derive(Clone, Debug)]
pub enum Signal {
    Connected,
    Changed(String),
    Disconnected(String),
}

impl Client {
    pub fn new(base: &str, token: &str) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(15)))
            .build()
            .into();
        Self {
            base: base.trim_end_matches('/').to_string(),
            token: token.to_string(),
            agent,
        }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        self.agent
            .get(format!("{}{path}", self.base))
            .header("Authorization", format!("Bearer {}", self.token))
            .call()
            .map_err(|e| format!("GET {path}: {e}"))?
            .body_mut()
            .read_json()
            .map_err(|e| format!("GET {path}: {e}"))
    }

    pub fn snapshot(&self, selected: Option<ScenarioId>) -> Result<Snapshot, String> {
        let scenarios: Vec<Scenario> = self.get("/v1/scenarios")?;
        let selected = selected
            .filter(|id| scenarios.iter().any(|s| s.id == *id))
            .or_else(|| scenarios.last().map(|s| s.id));
        let detail = match selected.and_then(|id| scenarios.iter().find(|s| s.id == id)) {
            None => None,
            Some(scenario) => {
                let id = scenario.id;
                let runs: Vec<Run> = self.get(&format!("/v1/scenarios/{id}/runs"))?;
                let results: Vec<ResultSummary> =
                    self.get(&format!("/v1/scenarios/{id}/results"))?;
                let shown = scenario.published_result.or_else(|| {
                    results
                        .iter()
                        .rev()
                        .find(|r| r.valid && matches!(r.status.as_str(), "proposed" | "approved"))
                        .map(|r| r.id)
                });
                let plan = match shown {
                    Some(r) => Some(self.get(&format!("/v1/results/{r}"))?),
                    None => None,
                };
                Some(Detail {
                    scenario: scenario.clone(),
                    runs,
                    results,
                    plan,
                })
            }
        };
        Ok(Snapshot { scenarios, detail })
    }

    /// Follow `/v1/events` forever, reconnecting after failures. Every event and
    /// reconnect asks the window to refresh; events carry no state themselves.
    pub fn follow(&self, signals: UnboundedSender<Signal>) {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(None)
            .build()
            .into();
        loop {
            let response = agent
                .get(format!("{}/v1/events", self.base))
                .header("Authorization", format!("Bearer {}", self.token))
                .header("Accept", "text/event-stream")
                .call();
            match response {
                Ok(response) => {
                    if signals.unbounded_send(Signal::Connected).is_err() {
                        return;
                    }
                    let reader = BufReader::new(response.into_body().into_reader());
                    for line in reader.lines() {
                        let Ok(line) = line else { break };
                        if let Some(kind) = line.strip_prefix("event:") {
                            let signal = Signal::Changed(kind.trim().to_string());
                            if signals.unbounded_send(signal).is_err() {
                                return;
                            }
                        }
                    }
                    let _ =
                        signals.unbounded_send(Signal::Disconnected("event stream closed".into()));
                }
                Err(e) => {
                    if signals
                        .unbounded_send(Signal::Disconnected(e.to_string()))
                        .is_err()
                    {
                        return;
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    }
}
