//! Jira Cloud as a `Connector` (phase 19, phase 4D-2): reading only so far; writing is the next
//! part, and until then a push is refused with `Unsupported`.

use std::sync::atomic::AtomicBool;

use sheafly_engine::fields::FieldDef;
use sheafly_engine::push::PushPlan;
use sheafly_engine::source::Source;
use sheafly_engine::Lang;

use crate::connector::{Connector, Converted, Progress, PushProgress, PushReport, WriteSetup};
use crate::jira::{self, project_key_ok};
use crate::jira_fields::{self, read_plan};
use crate::tracker::{Api, Credentials, TrackerError};

/// Jira Cloud sites live under `*.atlassian.net`. Only they are called, so a project file
/// naming another site cannot make the app send a token anywhere else.
pub fn is_jira_site(site: &str) -> bool {
    site.strip_suffix(".atlassian.net")
        .is_some_and(|label| !label.is_empty() && !label.contains(['.', '/', ':', '@']))
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding (RFC 4648), for the Basic authorization header.
fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(BASE64[(n >> 18) as usize & 63] as char);
        out.push(BASE64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { BASE64[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { BASE64[n as usize & 63] as char } else { '=' });
    }
    out
}

pub struct JiraConnector;

impl Connector for JiraConnector {
    fn allows_site(&self, site: &str) -> bool {
        is_jira_site(site)
    }

    fn needs_account(&self) -> bool {
        true
    }

    fn authorize(&self, credentials: &Credentials) -> Result<(String, String), TrackerError> {
        match credentials {
            Credentials::Basic { account, token } => {
                let encoded = base64(format!("{account}:{token}").as_bytes());
                Ok(("Authorization".to_string(), format!("Basic {encoded}")))
            }
            Credentials::ApiKey(_) => Err(TrackerError::NoKey),
        }
    }

    fn check(&self, api: &dyn Api, source: &Source, fields: &[FieldDef]) -> Result<String, TrackerError> {
        if !project_key_ok(&source.project_key) {
            return Err(TrackerError::BadProject);
        }
        let project = api.get(&format!("/rest/api/3/project/{}", source.project_key), &[])?;
        let name = project.get("name").and_then(|name| name.as_str()).unwrap_or(&source.project_key).to_string();
        // The settings' references are checked too, so a wrong name shows up here.
        read_plan(source, fields, &jira_fields::fields(api)?)?;
        Ok(name)
    }

    fn pull(
        &self,
        api: &dyn Api,
        source: &Source,
        fields: &[FieldDef],
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(Progress),
    ) -> Result<Converted, TrackerError> {
        let defs = jira_fields::fields(api)?;
        let reads = read_plan(source, fields, &defs)?;
        let issues = jira::fetch(api, source, &reads.request, cancel, progress)?;
        Ok(jira::convert(&issues, source, fields, &reads))
    }

    fn write_setup(
        &self,
        _api: &dyn Api,
        _source: &Source,
        _fields: &[FieldDef],
        _plan: &PushPlan,
    ) -> Result<WriteSetup, TrackerError> {
        Err(TrackerError::Unsupported)
    }

    fn write(
        &self,
        _api: &dyn Api,
        plan: &PushPlan,
        _setup: &WriteSetup,
        lang: Lang,
        _cancel: &AtomicBool,
        _progress: &mut dyn FnMut(PushProgress),
    ) -> PushReport {
        PushReport {
            not_sent: plan.changes.iter().map(|change| change.key.clone()).collect(),
            stopped: Some(TrackerError::Unsupported.message(lang).to_string()),
            ..PushReport::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use serde_json::{json, Value};

    use super::*;

    #[test]
    fn only_jira_cloud_sites_are_called() {
        for good in ["team.atlassian.net", "a-b.atlassian.net", "x1.atlassian.net"] {
            assert!(is_jira_site(good), "{good}");
        }
        for bad in [
            "atlassian.net",
            ".atlassian.net",
            "team.backlog.com",
            "team.atlassian.net.evil.com",
            "a.b.atlassian.net",
            "evil.com/team.atlassian.net",
            "user@team.atlassian.net",
            "team.atlassian.net:8080",
            "",
        ] {
            assert!(!is_jira_site(bad), "{bad}");
        }
    }

    #[test]
    fn base64_matches_the_rfc_vectors() {
        for (input, expected) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
            ("fred:fred", "ZnJlZDpmcmVk"),
        ] {
            assert_eq!(base64(input.as_bytes()), expected, "{input}");
        }
    }

    #[test]
    fn the_account_and_token_make_a_basic_header_and_a_lone_key_is_refused() {
        let connector = JiraConnector;
        assert!(connector.needs_account());
        let creds = Credentials::Basic { account: "fred".into(), token: "fred".into() };
        assert_eq!(connector.authorize(&creds), Ok(("Authorization".to_string(), "Basic ZnJlZDpmcmVk".to_string())));
        assert_eq!(connector.authorize(&Credentials::ApiKey("k".into())), Err(TrackerError::NoKey));
    }

    #[test]
    fn a_push_is_refused_for_now() {
        let connector = JiraConnector;
        let api = Quiet;
        let plan = PushPlan::default();
        assert_eq!(
            connector.write_setup(&api, &sample_source(), &[], &plan).unwrap_err(),
            TrackerError::Unsupported
        );
        let report = connector.write(&api, &plan, &WriteSetup::default(), Lang::En, &AtomicBool::new(false), &mut |_| {});
        assert!(report.done.is_empty());
        assert!(report.stopped.unwrap().contains("does not support"));
    }

    fn sample_source() -> Source {
        Source {
            kind: sheafly_engine::source::TrackerKind::Jira,
            site: "team.atlassian.net".into(),
            project_key: "PROJ".into(),
            filter: String::new(),
            hours_per_point: 4.0,
            status_map: vec![],
            sync: Default::default(),
        }
    }

    struct Quiet;
    impl Api for Quiet {
        fn get(&self, _: &str, _: &[(String, String)]) -> Result<Value, TrackerError> {
            Err(TrackerError::NotFound)
        }
        fn patch(&self, _: &str, _: &[(String, String)]) -> Result<Value, TrackerError> {
            Err(TrackerError::NotFound)
        }
    }

    /// A fake Jira with a project, the site's fields and one page of issues.
    struct Fake {
        seen: RefCell<Vec<String>>,
    }

    impl Api for Fake {
        fn get(&self, path: &str, _: &[(String, String)]) -> Result<Value, TrackerError> {
            self.seen.borrow_mut().push(path.to_string());
            match path {
                "/rest/api/3/project/PROJ" => Ok(json!({ "key": "PROJ", "name": "Dự án mẫu" })),
                "/rest/api/3/project/NOPE" => Err(TrackerError::NotFound),
                "/rest/api/3/field" => Ok(json!([
                    { "id": "duedate", "name": "Due date", "custom": false, "schema": { "type": "date" } },
                    { "id": "customfield_10015", "name": "Start date", "custom": true, "schema": { "type": "date" } }
                ])),
                _ => Err(TrackerError::NotFound),
            }
        }
        fn patch(&self, _: &str, _: &[(String, String)]) -> Result<Value, TrackerError> {
            Err(TrackerError::NotFound)
        }
        fn post_json(&self, path: &str, _: &Value) -> Result<Value, TrackerError> {
            match path {
                "/rest/api/3/search/jql" => Ok(serde_json::from_str(include_str!("../fixtures/jira_search.json")).unwrap()),
                _ => Err(TrackerError::NotFound),
            }
        }
    }

    #[test]
    fn check_names_the_project_and_checks_the_settings_against_the_sites_fields() {
        let api = Fake { seen: RefCell::new(Vec::new()) };
        assert_eq!(JiraConnector.check(&api, &sample_source(), &[]).unwrap(), "Dự án mẫu");
        let wrong = Source {
            sync: sheafly_engine::source::SyncSettings {
                fields: vec![sheafly_engine::source::FieldPolicy {
                    field: "start".into(),
                    direction: Default::default(),
                    on_conflict: Default::default(),
                    source_field: Some("custom:Không có".into()),
                    push_field: None,
                }],
                ..Default::default()
            },
            ..sample_source()
        };
        assert_eq!(JiraConnector.check(&api, &wrong, &[]).unwrap_err(), TrackerError::BadField);
        let bad_key = Source { project_key: "NOPE".into(), ..sample_source() };
        assert_eq!(JiraConnector.check(&api, &bad_key, &[]).unwrap_err(), TrackerError::NotFound);
        let unsafe_key = Source { project_key: "A/B".into(), ..sample_source() };
        assert_eq!(JiraConnector.check(&api, &unsafe_key, &[]).unwrap_err(), TrackerError::BadProject);
    }

    #[test]
    fn pull_reads_the_sites_fields_then_the_issues() {
        let api = Fake { seen: RefCell::new(Vec::new()) };
        let converted = JiraConnector.pull(&api, &sample_source(), &[], &AtomicBool::new(false), &mut |_| {}).unwrap();
        assert_eq!(converted.rows.len(), 3);
        assert_eq!(api.seen.borrow().as_slice(), ["/rest/api/3/field"]);
    }
}
