//! A Canon decision as the loop reads it: claim values, action statuses, outcome statuses.

use std::collections::BTreeMap;

use anyhow::{Context as _, Result};

/// The parts of a `canon-decision/1` document the loop acts on.
#[derive(Debug, Clone)]
pub struct Decision {
    /// Each claim's value: `true`, `false` or `unknown`.
    pub claims: BTreeMap<String, String>,
    /// Each obligation's status: `open`, `discharged` or `not_applicable`.
    pub obligations: BTreeMap<String, String>,
    /// Each action's status: `admissible`, `approval-required` or `blocked`.
    pub actions: BTreeMap<String, String>,
    /// Each outcome's status: `legitimate` or `blocked`.
    pub outcomes: BTreeMap<String, String>,
    /// The whole document, canonical JSON.
    pub json: String,
}

impl Decision {
    /// Reads the rendered decision document.
    pub fn from_json(json: &str) -> Result<Self> {
        let doc: serde_json::Value =
            serde_json::from_str(json).context("read the decision document")?;
        // A section is a map from id to entry, or a list of entries carrying their `id`.
        let statuses = |section: &str, key: &str| -> BTreeMap<String, String> {
            let value = |entry: &serde_json::Value| {
                entry[key]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| entry.to_string())
            };
            match &doc[section] {
                serde_json::Value::Object(map) => map
                    .iter()
                    .map(|(id, entry)| (id.clone(), value(entry)))
                    .collect(),
                serde_json::Value::Array(list) => list
                    .iter()
                    .filter_map(|entry| {
                        entry["id"].as_str().map(|id| (id.to_owned(), value(entry)))
                    })
                    .collect(),
                _ => BTreeMap::new(),
            }
        };
        Ok(Self {
            claims: statuses("claims", "value"),
            obligations: statuses("obligations", "status"),
            actions: statuses("actions", "status"),
            outcomes: statuses("outcomes", "status"),
            json: json.to_owned(),
        })
    }

    /// The admissible actions, in id order.
    #[must_use]
    pub fn admissible(&self) -> Vec<String> {
        self.actions
            .iter()
            .filter(|(_, status)| status.as_str() == "admissible")
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// The actions that need an approval, in id order.
    #[must_use]
    pub fn approval_required(&self) -> Vec<String> {
        self.actions
            .iter()
            .filter(|(_, status)| status.as_str() == "approval-required")
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// The legitimate outcomes, in id order.
    #[must_use]
    pub fn legitimate(&self) -> Vec<String> {
        self.outcomes
            .iter()
            .filter(|(_, status)| status.as_str() == "legitimate")
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Whether `claim` is `true`.
    #[must_use]
    pub fn holds(&self, claim: &str) -> bool {
        self.claims.get(claim).is_some_and(|value| value == "true")
    }

    /// One line: each action with its status.
    #[must_use]
    pub fn frontier_line(&self) -> String {
        let parts: Vec<String> = self
            .actions
            .iter()
            .map(|(id, status)| format!("{id} ({status})"))
            .collect();
        format!("frontier: {}", parts.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_are_read_from_each_section() {
        let json = r#"{"claims":{"tests.pass":{"value":"true"}},
            "obligations":[{"id":"tests.required","status":"discharged"}],
            "actions":{"tests.run":{"status":"admissible"},"repository.merge":{"status":"approval-required"}},
            "outcomes":{"accepted":{"status":"blocked"}}}"#;
        let decision = Decision::from_json(json).expect("reads");
        assert!(decision.holds("tests.pass"));
        assert_eq!(
            decision
                .obligations
                .get("tests.required")
                .map(String::as_str),
            Some("discharged")
        );
        assert_eq!(decision.admissible(), vec!["tests.run".to_owned()]);
        assert_eq!(
            decision.approval_required(),
            vec!["repository.merge".to_owned()]
        );
        assert!(decision.legitimate().is_empty());
        assert_eq!(
            decision.frontier_line(),
            "frontier: repository.merge (approval-required), tests.run (admissible)"
        );
    }
}
