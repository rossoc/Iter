//! The organization edit form.

use super::{DESCRIPTION, NAME, PROJECTS, Pairs, SettingsForm, text};
use crate::db::Db;
use crate::error::Result;
use crate::models::{Configured, Named, Organization};
use crate::web::edit::EditForm;
use crate::web::url::org_url;

/// What the organization edit form submits. The projects come as repeated
/// [`PROJECTS`] fields, one per ticked box, which a struct can't hold, so
/// the raw pairs are read instead ([`Self::parse`]).
pub struct OrgForm {
    pub name: String,
    pub description: String,
    pub settings: SettingsForm,
    /// The ids of the projects ticked.
    pub projects: Vec<i64>,
}

impl OrgForm {
    pub fn parse(pairs: &[(String, String)]) -> OrgForm {
        let pairs = Pairs(pairs);
        OrgForm {
            name: pairs.first(NAME),
            description: pairs.first(DESCRIPTION),
            settings: SettingsForm::parse(&pairs),
            projects: pairs.ids(PROJECTS),
        }
    }
}

impl EditForm for OrgForm {
    type Row = Organization;

    fn saved(id: i64) -> String {
        org_url(id)
    }

    fn apply(&self, mut org: Organization) -> (Organization, Result<()>) {
        org.name = self.name.trim().to_string();
        org.description = text(&self.description);
        org.set_settings(self.settings.settings());
        let valid = org.validate();
        (org, valid)
    }

    /// The projects in the organization too, in the same transaction.
    fn save(&self, db: &Db, id: i64, org: &Organization) -> Result<()> {
        db.save_group(id, org, &self.projects)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Settings;

    /// The organization form reads its settings and its repeated ticks from
    /// the pairs, decoded the way topcoat decodes a real submission: an
    /// unticked box is simply absent.
    #[test]
    fn an_org_submission_decodes_its_settings_and_ticks() {
        let body = "name=acme&description=&github=on&branch_template=fix%2F%7Btask%7D\
                    &default_branch=dev&github_project=Roadmap&project=7&project=3&project=7&project=x";
        let topcoat::router::content::Form(pairs) =
            topcoat::router::content::Form::<Vec<(String, String)>>::from_bytes(body.as_bytes())
                .expect("decodes");
        let form = OrgForm::parse(&pairs);
        let (org, valid) = form.apply(Organization::default());
        assert!(valid.is_ok());
        assert_eq!(
            org.settings(),
            Settings {
                github: true,
                tmux: false,
                auto_branch: false,
                branch_template: "fix/{task}".into(),
                default_branch: "dev".into(),
                github_project: "Roadmap".into(),
            }
        );
        assert_eq!(form.projects, [3, 7]);
    }
}
