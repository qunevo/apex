//! German and English display texts and number, time and duration formats.
use apex_control::{ResultStatus, RunState};
use chrono::{DateTime, Utc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    De,
    En,
}

impl Lang {
    /// German when the system locale asks for it, otherwise English.
    pub fn from_env() -> Self {
        let locale = ["APEX_LANG", "LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
            .unwrap_or_default();
        if locale.to_ascii_lowercase().starts_with("de") {
            Self::De
        } else {
            Self::En
        }
    }

    pub fn t(self, key: Text) -> &'static str {
        use Text::*;
        let (de, en) = match key {
            Published => ("Veröffentlicht", "Published"),
            Proposal => ("Vorschlag", "Proposal"),
            NoPlan => (
                "Noch kein validierter Plan. Starte einen Lauf über die Steuerungsebene.",
                "No validated plan yet. Start a run through the control platform.",
            ),
            NoScenarios => (
                "Noch keine Szenarien. Lege eines über die Steuerungsebene an (HTTP oder MCP).",
                "No scenarios yet. Create one through the control platform (HTTP or MCP).",
            ),
            Revision => ("Revision", "Revision"),
            OlderRevision => ("ältere Revision", "older revision"),
            Resource => ("Ressource", "Resource"),
            Operations => ("Arbeitsgänge", "Operations"),
            Resources => ("Ressourcen", "Resources"),
            Span => ("Zeitraum", "Span"),
            Activity => ("Aktivität", "Activity"),
            Runs => ("Läufe", "Runs"),
            Results => ("Ergebnisse", "Results"),
            Valid => ("gültig", "valid"),
            Invalid => ("ungültig", "invalid"),
            Live => ("Live", "Live"),
            Connecting => ("Verbinde …", "Connecting …"),
            Reconnecting => ("Verbindung unterbrochen", "Reconnecting"),
            DisplayOnly => (
                "Nur Anzeige · Änderungen kommen über die Steuerungsebene",
                "Display only · changes come through the control platform",
            ),
            HoverHint => (
                "Für Details mit der Maus über einen Arbeitsgang fahren",
                "Hover an operation for details",
            ),
            Start => ("Start", "Start"),
            End => ("Ende", "End"),
            Duration => ("Dauer", "Duration"),
            Mode => ("Modus", "Mode"),
            Fit => ("Einpassen", "Fit"),
            Engine => ("Engine", "Engine"),
            LastChange => ("Letzte Änderung", "Last change"),
        };
        match self {
            Self::De => de,
            Self::En => en,
        }
    }

    pub fn run_state(self, s: RunState) -> &'static str {
        match (self, s) {
            (Self::De, RunState::Queued) => "wartend",
            (Self::De, RunState::Running) => "läuft",
            (Self::De, RunState::Succeeded) => "erfolgreich",
            (Self::De, RunState::Failed) => "fehlgeschlagen",
            (Self::De, RunState::Cancelled) => "abgebrochen",
            (Self::En, s) => s.as_str(),
        }
    }

    pub fn result_status(self, s: ResultStatus) -> &'static str {
        match (self, s) {
            (Self::De, ResultStatus::Proposed) => "vorgeschlagen",
            (Self::De, ResultStatus::Approved) => "freigegeben",
            (Self::De, ResultStatus::Rejected) => "abgelehnt",
            (Self::De, ResultStatus::Published) => "veröffentlicht",
            (Self::De, ResultStatus::Superseded) => "ersetzt",
            (Self::En, s) => s.as_str(),
        }
    }

    pub fn number(self, v: f64) -> String {
        let rounded = if v.abs() >= 1000. {
            format!("{:.0}", v.abs())
        } else {
            format!("{:.1}", v.abs())
        };
        let (int, frac) = match rounded.split_once('.') {
            Some((i, f)) => (i.to_string(), Some(f.to_string()).filter(|f| f != "0")),
            None => (rounded, None),
        };
        let (group, decimal) = match self {
            Self::De => ('.', ','),
            Self::En => (',', '.'),
        };
        let mut grouped = String::new();
        for (i, c) in int.chars().enumerate() {
            if i > 0 && (int.len() - i) % 3 == 0 {
                grouped.push(group);
            }
            grouped.push(c);
        }
        let sign = if v < 0. { "-" } else { "" };
        match frac {
            Some(f) => format!("{sign}{grouped}{decimal}{f}"),
            None => format!("{sign}{grouped}"),
        }
    }

    pub fn duration(self, seconds: i64) -> String {
        let (d, h, m) = (
            seconds / 86_400,
            seconds % 86_400 / 3600,
            seconds % 3600 / 60,
        );
        let day = if self == Self::De { "T" } else { "d" };
        match (d, h, m) {
            (0, 0, 0) => format!("{seconds} s"),
            (0, 0, m) => format!("{m} min"),
            (0, h, 0) => format!("{h} h"),
            (0, h, m) => format!("{h} h {m} min"),
            (d, h, _) => format!("{d} {day} {h} h"),
        }
    }

    pub fn time(self, t: DateTime<Utc>, with_date: bool) -> String {
        let pattern = match (self, with_date) {
            (_, false) => "%H:%M",
            (Self::De, true) => "%d.%m. %H:%M",
            (Self::En, true) => "%b %-d, %H:%M",
        };
        t.format(pattern).to_string()
    }

    /// Readable metric names, e.g. `flow_time` → `Flow time`.
    pub fn metric(self, key: &str) -> String {
        let de = match key {
            "makespan" => Some("Gesamtdauer"),
            "flow_time" => Some("Durchlaufzeit"),
            "tardiness" | "total_tardiness" => Some("Verspätung"),
            "late_jobs" | "late_orders" => Some("Verspätete Aufträge"),
            "setup_time" => Some("Rüstzeit"),
            "idle_time" => Some("Leerlaufzeit"),
            "utilization" => Some("Auslastung"),
            "conditional_time" => Some("Nebenzeiten"),
            _ => None,
        };
        if let (Self::De, Some(name)) = (self, de) {
            return name.to_string();
        }
        let text = key.replace('_', " ");
        let mut chars = text.chars();
        chars
            .next()
            .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy)]
pub enum Text {
    Published,
    Proposal,
    NoPlan,
    NoScenarios,
    Revision,
    OlderRevision,
    Resource,
    Operations,
    Resources,
    Span,
    Activity,
    Runs,
    Results,
    Valid,
    Invalid,
    Live,
    Connecting,
    Reconnecting,
    DisplayOnly,
    HoverHint,
    Start,
    End,
    Duration,
    Mode,
    Fit,
    Engine,
    LastChange,
}

#[test]
fn formats_follow_the_language() {
    assert_eq!(Lang::De.number(40140.0), "40.140");
    assert_eq!(Lang::En.number(40140.0), "40,140");
    assert_eq!(Lang::De.number(12.5), "12,5");
    assert_eq!(Lang::En.number(-3.75), "-3.8");
    assert_eq!(Lang::En.number(12.96), "13");
    assert_eq!(Lang::De.duration(5400), "1 h 30 min");
    assert_eq!(Lang::En.duration(90_000), "1 d 1 h");
    assert_eq!(Lang::De.metric("flow_time"), "Durchlaufzeit");
    assert_eq!(Lang::En.metric("flow_time"), "Flow time");
}
