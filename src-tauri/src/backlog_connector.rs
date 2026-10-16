//! Backlog as a `Connector`: the existing adapter (`backlog.rs`, `backlog_fields.rs`,
//! `backlog_push.rs`) behind the trait.

use std::sync::atomic::AtomicBool;

use sheafly_engine::fields::FieldDef;
use sheafly_engine::push::PushPlan;
use sheafly_engine::source::Source;
use sheafly_engine::Lang;

use crate::backlog::{self, project};
use crate::backlog_fields::{plan_for, push_targets};
use crate::backlog_push;
use crate::connector::{Connector, Converted, Progress, PushProgress, PushReport, WriteSetup};
use crate::tracker::{Api, Credentials, TrackerError};

/// Backlog spaces live under these domains. Only they are called, so a project file naming
/// another site cannot make the app send a key anywhere else.
pub fn is_backlog_site(site: &str) -> bool {
    [".backlog.com", ".backlog.jp", ".backlogtool.com"].iter().any(|suffix| {
        site.strip_suffix(suffix)
            .is_some_and(|label| !label.is_empty() && !label.contains(['.', '/', ':', '@']))
    })
}

pub struct BacklogConnector;

impl Connector for BacklogConnector {
    fn allows_site(&self, site: &str) -> bool {
        is_backlog_site(site)
    }

    fn needs_account(&self) -> bool {
        false
    }

    fn authorize(&self, credentials: &Credentials) -> Result<(String, String), TrackerError> {
        match credentials {
            Credentials::ApiKey(key) => Ok(("Backlog-API-Key".to_string(), key.clone())),
            Credentials::Basic { .. } => Err(TrackerError::NoKey),
        }
    }

    fn check(&self, api: &dyn Api, source: &Source, fields: &[FieldDef]) -> Result<String, TrackerError> {
        let (_, name) = project(api, &source.project_key)?;
        // The settings' references are checked too, so a wrong name shows up here.
        plan_for(api, source, fields)?;
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
        let (reads, _) = plan_for(api, source, fields)?;
        let issues = backlog::fetch(api, source, cancel, progress)?;
        Ok(backlog::convert(&issues, source, fields, &reads))
    }

    fn write_setup(
        &self,
        api: &dyn Api,
        source: &Source,
        fields: &[FieldDef],
        plan: &PushPlan,
    ) -> Result<WriteSetup, TrackerError> {
        let (_, defs) = plan_for(api, source, fields)?;
        // A day sent to a date custom field of its own needs that field's id.
        let references: Vec<&str> = plan
            .changes
            .iter()
            .flat_map(|change| [change.start_field.as_deref(), change.due_field.as_deref()])
            .flatten()
            .collect();
        let fields = push_targets(&defs, &references)?
            .into_iter()
            .map(|(name, id)| (name, id.to_string()))
            .collect();
        let wants_people = plan.changes.iter().any(|change| change.assignee.is_some());
        let people = if wants_people { backlog_push::users(api, &source.project_key)? } else { Default::default() };
        Ok(WriteSetup { people, fields })
    }

    fn write(
        &self,
        api: &dyn Api,
        plan: &PushPlan,
        setup: &WriteSetup,
        lang: Lang,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(PushProgress),
    ) -> PushReport {
        backlog_push::push_issues(api, &plan.changes, &setup.people, &setup.fields, lang, cancel, progress)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_backlog_spaces_are_called() {
        for good in ["team.backlog.com", "a-b.backlog.jp", "x1.backlogtool.com"] {
            assert!(is_backlog_site(good), "{good}");
        }
        for bad in [
            "backlog.com",
            ".backlog.com",
            "evil.com",
            "team.backlog.com.evil.com",
            "a.b.backlog.com",
            "evil.com/team.backlog.com",
            "user@team.backlog.com",
            "team.backlog.com:8080",
            "",
        ] {
            assert!(!is_backlog_site(bad), "{bad}");
        }
    }

    #[test]
    fn the_key_goes_in_the_backlog_header_and_an_account_is_not_expected() {
        let connector = BacklogConnector;
        assert!(!connector.needs_account());
        assert_eq!(
            connector.authorize(&Credentials::ApiKey("k".into())),
            Ok(("Backlog-API-Key".to_string(), "k".to_string()))
        );
        assert_eq!(
            connector.authorize(&Credentials::Basic { account: "a".into(), token: "t".into() }),
            Err(TrackerError::NoKey)
        );
    }
}

