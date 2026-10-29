//! The governor of Loom decides takt's recorded cases as takt decided them
//! (`story:loom-sdk-pinned-with-evaluation-parity`).
//!
//! Every decision takt wrote under `.takt/cases/turn-*/decisions/` is decided again by
//! `loom_sdk::governor::evaluate`, from the request the decision's own
//! `explanation.computed_from` names (the case snapshot, the evidence ids, the instant), under
//! the case's own `protocol.yaml`. The two are compared as parsed values. A decision that read no
//! grant must come back equal. A decision that read one may differ only where the missing grant
//! puts it, which [`allowed`] names; any other difference in any decision fails.
//!
//! A decision names the Canon release that rendered it (`explanation.computed_from.semantics
//! .canon`). Each recorded decision must name [`RECORDED_CANON`] and the governor's must name the
//! `b10x-canon` that `Cargo.lock` holds; the recorded stamp is then read as the governor's, and
//! every other member is compared as above.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use loom_sdk::governor::evaluate;
use loom_sdk::governor::model::evaluation::{
    CaseSnapshot, EvaluationRequest, EvidenceRecord, ProtocolName,
};
use loom_sdk::governor::model::json;
use loom_sdk::governor::model::primitives::Timestamp;
use loom_sdk::protocols::{ProtocolCatalog, ProtocolSource, SourceKind};
use serde_json::Value;

/// The recorded cases, each with its own `protocol.yaml`.
const CASES: [&str; 3] = ["turn-1-scenario-help", "turn-2-case-id", "turn-3-hours"];

/// The name the case's protocol is registered under in the catalog. The protocol's id is
/// `self.improvement`, revision 1; `ProtocolCatalog::add_yaml` takes a registration name of
/// lowercase letters, digits and hyphens only, so `self.improvement@1` is refused, and the
/// registration name is not part of Canon's decision (its `protocol` is the document's id).
const REGISTERED_AS: &str = "self-improvement@1";

/// The Canon release every recorded decision names: the one takt linked when it decided them.
const RECORDED_CANON: &str = "0.6.0";

fn cases_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.engineering/cases")
}

/// The version of the one `b10x-canon` package `Cargo.lock` holds.
fn locked_canon() -> String {
    let lock =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock"))
            .expect("read Cargo.lock");
    let marker = "name = \"b10x-canon\"\n";
    assert_eq!(
        lock.matches(marker).count(),
        1,
        "Cargo.lock holds one b10x-canon"
    );
    let at = lock.find(marker).expect("Cargo.lock holds b10x-canon") + marker.len();
    lock[at..]
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("version = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .expect("b10x-canon's version")
        .to_owned()
}

/// Checks the Canon release each side names, the recorded decision [`RECORDED_CANON`] and the
/// governor's the locked `b10x-canon`, and answers the recorded decision with the governor's
/// stamp in its place.
fn with_governors_canon(mut takt: Value, loom: &Value, canon: &str, name: &str) -> Value {
    let recorded = &mut takt["explanation"]["computed_from"]["semantics"]["canon"];
    assert_eq!(
        recorded, RECORDED_CANON,
        "{name}: the Canon the record names"
    );
    let rendered = &loom["explanation"]["computed_from"]["semantics"]["canon"];
    assert_eq!(rendered, canon, "{name}: the Canon the governor names");
    *recorded = rendered.clone();
    takt
}

/// One catalog per case: the cases' `protocol.yaml` files differ while sharing one id, and the
/// catalog refuses a second registration of one name.
fn catalog_of(case: &Path) -> ProtocolCatalog {
    let yaml = std::fs::read_to_string(case.join("protocol.yaml")).expect("read protocol.yaml");
    let mut catalog = ProtocolCatalog::default();
    catalog
        .add_yaml(
            REGISTERED_AS,
            &yaml,
            ProtocolSource {
                kind: SourceKind::Local,
                location: case.display().to_string(),
                revision: "1".into(),
                path: "protocol.yaml".into(),
            },
        )
        .expect("the catalog takes the case's protocol");
    catalog
}

/// The case's evidence records, by id, each as Loom's JSON reads its file.
fn evidence_of(case: &Path) -> BTreeMap<String, json::Value> {
    let mut records = BTreeMap::new();
    for entry in std::fs::read_dir(case.join("evidence")).expect("read evidence/") {
        let path = entry.expect("evidence entry").path();
        let text = std::fs::read_to_string(&path).expect("read an evidence record");
        let id = serde_json::from_str::<Value>(&text).expect("an evidence record is JSON")["id"]
            .as_str()
            .expect("an evidence record has an id")
            .to_owned();
        let value = json::parse(&text).expect("Loom's JSON reads the record");
        assert!(
            records.insert(id, value).is_none(),
            "{}: an id twice",
            path.display()
        );
    }
    records
}

fn to_loom(value: &Value) -> json::Value {
    json::parse(&serde_json::to_string(value).expect("render")).expect("Loom's JSON reads it")
}

fn from_loom(value: &json::Value) -> Value {
    let mut text = String::new();
    json::push_value(&mut text, value);
    serde_json::from_str(&text).expect("serde reads Loom's JSON")
}

/// One recorded decision: where it is, what takt wrote, and what the governor decides from the
/// request its `computed_from` names.
struct Fixture {
    name: String,
    takt: Value,
    loom: Value,
    /// The capabilities takt's decision read a grant for (`computed_from.authority`).
    granted: BTreeSet<String>,
    /// Each action the governor decided and the capabilities it requires.
    requires: BTreeMap<String, Vec<String>>,
}

fn fixtures() -> Vec<Fixture> {
    let canon = locked_canon();
    let mut out = Vec::new();
    for name in CASES {
        let case = cases_root().join(name);
        let catalog = catalog_of(&case);
        let evidence = evidence_of(&case);
        let mut decisions: Vec<PathBuf> = std::fs::read_dir(case.join("decisions"))
            .expect("read decisions/")
            .map(|entry| entry.expect("decision entry").path())
            .collect();
        decisions.sort();
        for path in decisions {
            let takt: Value =
                serde_json::from_str(&std::fs::read_to_string(&path).expect("read a decision"))
                    .expect("a decision is JSON");
            let from = &takt["explanation"]["computed_from"];
            let request = EvaluationRequest {
                protocol: ProtocolName(REGISTERED_AS.into()),
                snapshot: CaseSnapshot(to_loom(&from["case"])),
                evidence: from["evidence"]
                    .as_array()
                    .expect("computed_from.evidence")
                    .iter()
                    .map(|id| {
                        let id = id.as_str().expect("an evidence id");
                        EvidenceRecord(
                            evidence
                                .get(id)
                                .unwrap_or_else(|| panic!("{}: no record {id}", path.display()))
                                .clone(),
                        )
                    })
                    .collect(),
                at: Some(Timestamp(
                    from["at"].as_str().expect("computed_from.at").to_owned(),
                )),
                decisions: Vec::new(),
            };
            let decided = evaluate(&catalog, &request)
                .unwrap_or_else(|refusal| panic!("{}: refused: {refusal:?}", path.display()));
            let granted = from["authority"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|grant| grant["decision"] == "granted")
                .map(|grant| {
                    grant["capability"]
                        .as_str()
                        .expect("a capability")
                        .to_owned()
                })
                .collect();
            let name = path
                .strip_prefix(cases_root())
                .unwrap_or(&path)
                .display()
                .to_string();
            let loom = from_loom(&decided.canon);
            out.push(Fixture {
                takt: with_governors_canon(takt, &loom, &canon, &name),
                name,
                loom,
                granted,
                requires: decided
                    .actions
                    .iter()
                    .map(|action| (action.action.clone(), action.requires.clone()))
                    .collect(),
            });
        }
    }
    out
}

/// A place in a decision: the member names from its root.
type At = Vec<String>;

/// Every place where `takt` and `loom` differ: a member one side lacks, a scalar that differs,
/// or an array whose length or any item differs (the array itself).
fn differences(takt: &Value, loom: &Value, at: &mut At, out: &mut Vec<At>) {
    match (takt, loom) {
        (Value::Object(a), Value::Object(b)) => {
            let names: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
            for name in names {
                at.push(name.clone());
                match (a.get(name), b.get(name)) {
                    (Some(x), Some(y)) => differences(x, y, at, out),
                    _ => out.push(at.clone()),
                }
                at.pop();
            }
        }
        _ if takt != loom => out.push(at.clone()),
        _ => {}
    }
}

/// Whether a difference follows from the grant takt read and the governor does not: the
/// decision's `explanation.computed_from.authority`, and for an action that requires only
/// capabilities takt read granted, the action's status and reasons (`actions.<action>.status`,
/// `actions.<action>.reasons`) and its explanation (`explanation.actions.<action>`, which holds
/// the same status and reasons as `because`). Nothing else.
fn allowed(at: &[String], fixture: &Fixture) -> bool {
    let granted_action = |action: &String| {
        fixture.requires.get(action).is_some_and(|requires| {
            !requires.is_empty() && requires.iter().all(|c| fixture.granted.contains(c))
        })
    };
    match at {
        [e, c, a] if e == "explanation" && c == "computed_from" && a == "authority" => {
            !fixture.granted.is_empty()
        }
        [actions, action, field]
            if actions == "actions" && (field == "status" || field == "reasons") =>
        {
            granted_action(action)
        }
        [e, actions, action] if e == "explanation" && actions == "actions" => {
            granted_action(action)
        }
        _ => false,
    }
}

/// The differences of one fixture that the missing grant does not explain, each as a message;
/// and for each action allowed to differ, that takt shows it admissible and the governor
/// `approval-required`.
fn unexplained(fixture: &Fixture) -> Vec<String> {
    let mut found = Vec::new();
    differences(&fixture.takt, &fixture.loom, &mut Vec::new(), &mut found);
    let mut problems = Vec::new();
    for at in &found {
        let path = at.join(".");
        if !allowed(at, fixture) {
            problems.push(format!(
                "{}: {path}: takt {} / loom {}",
                fixture.name,
                pointer(&fixture.takt, at),
                pointer(&fixture.loom, at)
            ));
            continue;
        }
        let action = match at.as_slice() {
            [actions, action, ..] if actions == "actions" => action,
            [e, actions, action, ..] if e == "explanation" && actions == "actions" => action,
            _ => continue,
        };
        let loom_action = &fixture.loom["actions"][action];
        let takt = &fixture.takt["actions"][action]["status"];
        if takt != "admissible" || loom_action["status"] != "approval-required" {
            problems.push(format!(
                "{}: actions.{action}: takt {takt} / loom {}, not admissible / approval-required",
                fixture.name, loom_action["status"]
            ));
        }
        // The governor's reasons are exactly one `none` decision per capability the action
        // requires, as a set, and its explanation holds the same status and reasons.
        let mut expected: Vec<String> = fixture.requires[action]
            .iter()
            .map(|c| serde_json::json!({"capability": c, "decision": "none"}).to_string())
            .collect();
        expected.sort();
        let mut given: Vec<String> = loom_action["reasons"]
            .as_array()
            .map(|reasons| reasons.iter().map(Value::to_string).collect())
            .unwrap_or_default();
        given.sort();
        if given != expected {
            problems.push(format!(
                "{}: actions.{action}.reasons: loom {}, not a none decision per required capability",
                fixture.name, loom_action["reasons"]
            ));
        }
        let explained = serde_json::json!({
            "status": loom_action["status"],
            "because": loom_action["reasons"],
        });
        if fixture.loom["explanation"]["actions"][action] != explained {
            problems.push(format!(
                "{}: explanation.actions.{action}: loom {}, not its action's status and reasons",
                fixture.name, fixture.loom["explanation"]["actions"][action]
            ));
        }
    }
    problems.sort();
    problems.dedup();
    problems
}

fn pointer(value: &Value, at: &[String]) -> String {
    let mut here = Some(value);
    for name in at {
        here = here.and_then(|v| v.get(name));
    }
    here.map_or_else(|| "(absent)".into(), Value::to_string)
}

#[test]
fn loom_compiles_the_ir_takt_holds() {
    for name in CASES {
        let case = cases_root().join(name);
        let catalog = catalog_of(&case);
        let entry = catalog.get(REGISTERED_AS).expect("registered");
        let loom = b10x_canon::ir::compile(&entry.model)
            .expect("compiles")
            .canonical_json();
        let takt =
            std::fs::read_to_string(case.join("protocol.ir.json")).expect("read protocol.ir.json");
        let loom: Value = serde_json::from_str(&loom).expect("Loom's IR is JSON");
        let takt: Value = serde_json::from_str(&takt).expect("takt's IR is JSON");
        let mut found = Vec::new();
        differences(&takt, &loom, &mut Vec::new(), &mut found);
        assert!(
            found.is_empty(),
            "{name}: Loom's IR differs from protocol.ir.json first at {}",
            found[0].join(".")
        );
    }
}

#[test]
fn the_governor_decides_every_recorded_decision_as_takt_did() {
    let fixtures = fixtures();
    assert_eq!(fixtures.len(), 30, "the recorded decisions");
    let granted = fixtures.iter().filter(|f| !f.granted.is_empty()).count();
    assert_eq!(
        (fixtures.len() - granted, granted),
        (17, 13),
        "no grant / grant"
    );
    // A decision that read no grant allows no difference at all (`allowed` needs a grant).
    let problems: Vec<String> = fixtures.iter().flat_map(unexplained).collect();
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    for fixture in fixtures.iter().filter(|f| f.granted.is_empty()) {
        assert_eq!(fixture.takt, fixture.loom, "{}", fixture.name);
    }
}

#[test]
fn a_granted_decision_differs_exactly_where_the_grant_is_missing() {
    let fixtures = fixtures();
    let fixture = fixtures
        .iter()
        .find(|f| f.name.ends_with("turn-1-scenario-help/decisions/0007.json"))
        .expect("the granted fixture");
    assert_eq!(
        fixture.granted,
        BTreeSet::from([
            "repository.merge.judge_surface".to_owned(),
            "takt.promote".to_owned()
        ])
    );
    let mut found = Vec::new();
    differences(&fixture.takt, &fixture.loom, &mut Vec::new(), &mut found);
    let found: BTreeSet<String> = found.iter().map(|at| at.join(".")).collect();
    let expected: BTreeSet<String> = [
        "explanation.computed_from.authority",
        "actions.binary.promote.status",
        "actions.binary.promote.reasons",
        "explanation.actions.binary.promote",
        "actions.candidate.merge_judge_surface.status",
        "actions.candidate.merge_judge_surface.reasons",
        "explanation.actions.candidate.merge_judge_surface",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(found, expected);
    for action in ["binary.promote", "candidate.merge_judge_surface"] {
        assert_eq!(fixture.takt["actions"][action]["status"], "admissible");
        assert_eq!(
            fixture.loom["actions"][action]["status"],
            "approval-required"
        );
    }
    assert!(
        unexplained(fixture).is_empty(),
        "{:?}",
        unexplained(fixture)
    );
}

/// The comparison goes red on a difference the missing grant does not explain, planted in a
/// granted and in an ungranted decision's expected value.
#[test]
fn a_planted_difference_elsewhere_goes_red() {
    let fixtures = fixtures();
    let pick = |suffix: &str| {
        fixtures
            .iter()
            .find(|f| f.name.ends_with(suffix))
            .expect("fixture")
    };
    type Plant = fn(&mut Value);
    let plants: [(&str, Plant); 4] = [
        ("claim value", |d| {
            d["claims"]["fix.merged"]["value"] = Value::from("false");
        }),
        ("extra obligation", |d| {
            d["obligations"]
                .as_array_mut()
                .expect("obligations")
                .push(serde_json::json!({"id": "planted.required", "status": "open"}));
        }),
        ("status of an action needing no grant", |d| {
            d["actions"]["finding.file"]["status"] = Value::from("blocked");
        }),
        ("computed_from.at", |d| {
            d["explanation"]["computed_from"]["at"] = Value::from("2026-01-01T00:00:00Z");
        }),
    ];
    for suffix in [
        "turn-1-scenario-help/decisions/0007.json",
        "turn-2-case-id/decisions/0002.json",
    ] {
        let fixture = pick(suffix);
        for (what, plant) in plants {
            let mut takt = fixture.takt.clone();
            plant(&mut takt);
            let planted = Fixture {
                name: fixture.name.clone(),
                takt,
                loom: fixture.loom.clone(),
                granted: fixture.granted.clone(),
                requires: fixture.requires.clone(),
            };
            assert!(
                !unexplained(&planted).is_empty(),
                "{suffix}: a planted {what} is not caught"
            );
        }
    }
}

/// The governor's side of a granted action may differ only by the missing grant: an extra member
/// in its explanation, or a reason other than a `none` decision per required capability, goes red.
#[test]
fn a_planted_difference_in_a_granted_action_goes_red() {
    let fixtures = fixtures();
    let fixture = fixtures
        .iter()
        .find(|f| f.name.ends_with("turn-1-scenario-help/decisions/0007.json"))
        .expect("fixture");
    let granted = fixture
        .requires
        .iter()
        .find(|(_, requires)| {
            !requires.is_empty() && requires.iter().all(|c| fixture.granted.contains(c))
        })
        .map(|(action, _)| action.clone())
        .expect("a granted action");
    type Plant = fn(&mut Value, &str);
    let plants: [(&str, Plant); 2] = [
        ("an extra member in its explanation", |d, action| {
            d["explanation"]["actions"][action]["extra"] = Value::from(1);
        }),
        ("an unrelated reason", |d, action| {
            d["actions"][action]["reasons"] = serde_json::json!(["planted unrelated reason"]);
        }),
    ];
    assert!(unexplained(fixture).is_empty(), "the fixture itself passes");
    for (what, plant) in plants {
        let mut loom = fixture.loom.clone();
        plant(&mut loom, &granted);
        let planted = Fixture {
            name: fixture.name.clone(),
            takt: fixture.takt.clone(),
            loom,
            granted: fixture.granted.clone(),
            requires: fixture.requires.clone(),
        };
        assert!(
            !unexplained(&planted).is_empty(),
            "{granted}: a planted {what} is not caught"
        );
    }
}

fn stamped(canon: &str) -> Value {
    serde_json::json!({"explanation": {"computed_from": {"semantics": {"canon": canon}}}})
}

/// A recorded decision that names a Canon other than [`RECORDED_CANON`] goes red.
#[test]
#[should_panic(expected = "the Canon the record names")]
fn a_record_naming_another_canon_goes_red() {
    let canon = locked_canon();
    with_governors_canon(stamped("0.5.0"), &stamped(&canon), &canon, "planted");
}

/// A governor decision that names a Canon other than the locked one goes red.
#[test]
#[should_panic(expected = "the Canon the governor names")]
fn a_governor_naming_another_canon_goes_red() {
    let canon = locked_canon();
    with_governors_canon(
        stamped(RECORDED_CANON),
        &stamped("0.5.0"),
        &canon,
        "planted",
    );
}
