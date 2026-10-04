use apex::{server::config::Config, service::Service, transport};
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

struct Fixture {
    dir: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "apex-config-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        for id in ["alpha", "beta"] {
            let folder = dir.join("packages").join(id);
            fs::create_dir_all(&folder).unwrap();
            fs::write(
                folder.join("package.json"),
                json!({"id":id,"version":"1"}).to_string(),
            )
            .unwrap();
        }
        let this = Self { dir };
        this.config(json!({"customization_root":"packages","default_customization":"alpha","enabled_customizations":["alpha","beta"]}));
        this
    }
    fn config(&self, value: Value) {
        fs::write(self.dir.join("apex.config.json"), value.to_string()).unwrap();
    }
    fn service(&self) -> Service {
        Service::new(self.dir.join("state"), &self.dir)
            .unwrap()
            .with_config(&self.dir.join("apex.config.json"))
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.dir).unwrap();
    }
}

#[test]
fn request_selection_isolated_and_pinned_across_restart_and_viewer_links() {
    let f = Fixture::new();
    let mut s = f.service();
    s.viewer_url = "https://apex.example".into();
    let a = s.call("demo.create", &json!({"tasks":4})).unwrap();
    let b = s
        .call("demo.create", &json!({"customization":"beta","tasks":7}))
        .unwrap();
    assert_eq!(a["customization_package"]["id"], "alpha");
    assert_eq!(b["customization_package"]["id"], "beta");
    assert_eq!(
        s.call("scenario.get", &json!({"scenario_id":a["scenario_id"]}))
            .unwrap()["tasks"],
        4
    );
    assert!(
        s.call(
            "scenario.get",
            &json!({"customization":"beta","scenario_id":a["scenario_id"]})
        )
        .is_err()
    );
    assert!(
        s.call("scenario.get", &json!({"scenario_id":b["scenario_id"]}))
            .is_err()
    );
    let plan = s
        .call(
            "schedule.create",
            &json!({"customization":"beta","scenario_id":b["scenario_id"]}),
        )
        .unwrap();
    assert!(
        plan["viewer_url"]
            .as_str()
            .unwrap()
            .starts_with("https://apex.example/?schedule=")
    );
    assert!(
        plan["viewer_url"]
            .as_str()
            .unwrap()
            .ends_with("&customization=beta")
    );
    let saved: Value = serde_json::from_slice(
        &fs::read(
            f.dir
                .join("state/customization/beta/1")
                .join(format!("{}.json", plan["schedule_id"].as_str().unwrap())),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        saved["customization_package"],
        json!({"id":"beta","version":"1"})
    );
    let restarted = f.service();
    assert_eq!(
        restarted
            .call(
                "schedule.validate",
                &json!({"customization":"beta","schedule_id":plan["schedule_id"]})
            )
            .unwrap()["valid"],
        true
    );
    let fork = restarted
        .call(
            "scenario.fork",
            &json!({"customization":"beta","scenario_id":b["scenario_id"]}),
        )
        .unwrap();
    assert_eq!(fork["customization_package"]["id"], "beta");
    let caps = s.call("capabilities", &json!({})).unwrap();
    assert_eq!(caps["default_customization"], "alpha");
    assert_eq!(caps["enabled_customizations"], json!(["alpha", "beta"]));
}

#[test]
fn inputs_chunks_and_retained_artifacts_cannot_cross_package_boundaries() {
    let f = Fixture::new();
    let s = f.service();
    let mut problem = serde_json::to_value(apex::demo::problem(2)).unwrap();
    fs::write(f.dir.join("inputs/beta/source.json"), problem.to_string()).unwrap();
    assert!(
        s.call("problem.import", &json!({"path":"../beta/source.json"}))
            .is_err()
    );
    assert!(
        s.call(
            "problem.import",
            &json!({"customization":"beta","path":"source.json"})
        )
        .is_ok()
    );
    let tasks = problem["tasks"].take();
    problem["tasks"] = json!([]);
    let import = s
        .call(
            "import.begin",
            &json!({"customization":"beta","problem":problem,"expected_tasks":2}),
        )
        .unwrap();
    assert!(
        s.call("import.status", &json!({"import_id":import["import_id"]}))
            .is_err()
    );
    s.call("import.append", &json!({"customization":"beta","import_id":import["import_id"],"chunk_id":"first","tasks":tasks})).unwrap();
    let result = s
        .call(
            "import.finalize",
            &json!({"customization":"beta","import_id":import["import_id"]}),
        )
        .unwrap();
    assert_eq!(result["customization_package"]["id"], "beta");
    let (retained, failed) = s.agent_response(Ok(
        json!({"customization_package":{"id":"beta","version":"1"},"items":["x".repeat(70000)]}),
    ));
    assert!(!failed);
    assert!(
        s.call(
            "artifact.read",
            &json!({"artifact_id":retained["artifact_id"]})
        )
        .is_err()
    );
    assert!(
        s.call(
            "artifact.read",
            &json!({"customization":"beta","artifact_id":retained["artifact_id"]})
        )
        .is_ok()
    );
}

#[test]
fn invalid_selection_and_manifest_configuration_fail_closed() {
    let f = Fixture::new();
    let s = f.service();
    for value in [json!("missing"), json!("../beta"), json!(null), json!(12)] {
        assert_eq!(
            s.call("demo.create", &json!({"customization":value}))
                .unwrap_err()["code"],
            "CUSTOMIZATION"
        );
    }
    for (default, enabled) in [
        ("missing", json!(["alpha"])),
        ("alpha", json!(["alpha", "alpha"])),
        ("alpha", json!(["alpha", "../beta"])),
        ("alpha", json!([])),
    ] {
        f.config(json!({"customization_root":"packages","default_customization":default,"enabled_customizations":enabled}));
        assert!(Config::load(&f.dir.join("apex.config.json")).is_err());
    }
    f.config(json!({"customization_root":"packages","default_customization":"alpha","enabled_customizations":["alpha"]}));
    fs::write(
        f.dir.join("packages/alpha/package.json"),
        json!({"id":"beta","version":"1"}).to_string(),
    )
    .unwrap();
    assert!(Config::load(&f.dir.join("apex.config.json")).is_err());
    let plain = Service::new(f.dir.join("legacy"), &f.dir).unwrap();
    assert!(
        plain
            .call("demo.create", &json!({"customization":"alpha"}))
            .is_err()
    );
    assert!(plain.call("demo.create", &json!({"tasks":2})).is_ok());
}

#[test]
fn mcp_tools_advertise_and_route_the_same_context() {
    let f = Fixture::new();
    let s = f.service();
    for tool in transport::tools()["tools"].as_array().unwrap() {
        assert_eq!(
            tool["inputSchema"]["properties"]["customization"]["type"],
            "string"
        );
    }
    let response = transport::rpc(&s, &json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"demo.create","arguments":{"customization":"beta","tasks":3}}})).unwrap();
    assert_eq!(
        response["result"]["structuredContent"]["customization_package"]["id"],
        "beta"
    );
}
