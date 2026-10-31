//! Queries over the groups of projects: boards and organizations.

use super::*;

impl Db {
    /// Every project in group `id` (a board, an organization), in name order.
    pub fn projects_in<G: ProjectGroup>(&self, id: i64) -> Result<Vec<Project>> {
        self.find_all(
            &format!("WHERE {} = ?1 ORDER BY name", G::MEMBER_COLUMN),
            params![id],
        )
    }

    /// Every organization by name with its first `limit` projects and total project count; one
    /// query (window functions). Organizations with no projects are included.
    #[cfg(feature = "web")]
    pub fn organizations_with_projects(
        &self,
        limit: usize,
    ) -> Result<Vec<(Organization, Vec<Project>, usize)>> {
        let width = Project::COLUMNS.len();
        let sql = format!(
            "SELECT ranked.*, o.id, o.name
             FROM organizations o
             LEFT JOIN (
                 SELECT id, {},
                        COUNT(*) OVER (PARTITION BY organization_id) AS total,
                        ROW_NUMBER() OVER (PARTITION BY organization_id ORDER BY name) AS rn
                 FROM projects
                 WHERE organization_id IS NOT NULL
             ) ranked ON ranked.organization_id = o.id AND ranked.rn <= ?1
             ORDER BY o.name, ranked.name",
            Project::COLUMNS.join(", ")
        );
        // Columns: the project's `id` and columns (0..=width), its total, its
        // row number, then the organization's id and name.
        let rows = self.query_all(&sql, params![limit as i64], |row| {
            let org = Organization {
                id: Some(row.get(width + 3)?),
                name: row.get(width + 4)?,
                ..Organization::default()
            };
            // An organization without projects has NULLs where a project is.
            let project = if row.get::<_, Option<i64>>(0)?.is_some() {
                Some((
                    Project::from_row(row)?,
                    row.get::<_, i64>(width + 1)? as usize,
                ))
            } else {
                None
            };
            Ok((org, project))
        })?;
        let mut groups: Vec<(Organization, Vec<Project>, usize)> = Vec::new();
        for (org, project) in rows {
            if groups.last().map(|g| g.0.id) != Some(org.id) {
                groups.push((org, Vec::new(), 0));
            }
            if let Some((project, total)) = project {
                let group = groups
                    .last_mut()
                    .expect("pushed for this organization above");
                group.1.push(project);
                group.2 = total;
            }
        }
        Ok(groups)
    }

    /// The first `limit` projects in no organization, by name, and how many
    /// there are in all.
    #[cfg(feature = "web")]
    pub fn loose_projects(&self, limit: usize) -> Result<(Vec<Project>, usize)> {
        self.first_by_name("WHERE organization_id IS NULL", limit)
    }

    /// First `limit` rows of `T` matching `filter` (a `WHERE` clause or empty) by name, and the
    /// total match count: a window computed before the `LIMIT`.
    #[cfg(feature = "web")]
    pub fn first_by_name<T: Table>(&self, filter: &str, limit: usize) -> Result<(Vec<T>, usize)> {
        let sql = format!(
            "SELECT id, {}, COUNT(*) OVER () FROM {} {filter} ORDER BY name LIMIT ?1",
            T::COLUMNS.join(", "),
            T::NAME
        );
        let rows = self.query_all(&sql, params![limit as i64], |row| {
            let total: i64 = row.get(T::COLUMNS.len() + 1)?;
            Ok((T::from_row(row)?, total as usize))
        })?;
        let total = rows.first().map_or(0, |(_, total)| *total);
        Ok((rows.into_iter().map(|(item, _)| item).collect(), total))
    }

    /// Every project's id, name and `G` id in name order: a checklist's three columns.
    #[cfg(feature = "web")]
    pub fn project_owners<G: ProjectGroup>(&self) -> Result<Vec<(i64, String, Option<i64>)>> {
        self.query_all(
            &format!(
                "SELECT id, name, {} FROM projects ORDER BY name",
                G::MEMBER_COLUMN
            ),
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
    }

    /// Makes `project_ids` exactly group `id`'s projects: listed ones move in, the rest move out
    /// to none. Only membership changes.
    #[cfg(test)]
    pub fn set_projects<G: ProjectGroup>(&self, id: i64, project_ids: &[i64]) -> Result<()> {
        self.atomically(|| self.assign_projects::<G>(id, project_ids))
    }

    /// The membership write for a caller with a transaction open. Ids travel as one JSON array
    /// (`json_each`); repeated or unknown ids are harmless.
    pub(super) fn assign_projects<G: ProjectGroup>(
        &self,
        id: i64,
        project_ids: &[i64],
    ) -> Result<()> {
        let column = G::MEMBER_COLUMN;
        let ids = json_ids(project_ids);
        self.conn
            .prepare_cached(&format!(
                "UPDATE projects SET {column} = NULL WHERE {column} = ?1
                 AND id NOT IN (SELECT value FROM json_each(?2))"
            ))?
            .execute(params![id, ids])?;
        self.conn
            .prepare_cached(&format!(
                "UPDATE projects SET {column} = ?1 WHERE {column} IS NOT ?1
                 AND id IN (SELECT value FROM json_each(?2))"
            ))?
            .execute(params![id, ids])?;
        Ok(())
    }

    /// Writes `group` (a board, an organization) over row `id` and makes
    /// `project_ids` exactly its projects, in one transaction: a refused
    /// name leaves the projects as they were.
    pub fn save_group<G: ProjectGroup + Table>(
        &self,
        id: i64,
        group: &G,
        project_ids: &[i64],
    ) -> Result<()> {
        self.atomically(|| {
            self.update(id, group)?;
            self.assign_projects::<G>(id, project_ids)
        })
    }

    /// Every task of every project in group `id` with its project's name, by project then task;
    /// two queries.
    #[cfg_attr(not(feature = "web"), allow(dead_code))]
    pub fn tasks_in<G: ProjectGroup>(&self, id: i64) -> Result<Vec<(String, Task)>> {
        let mut tasks = self.tasks_of::<G>(id, false)?;
        Self::sort_by_project(&mut tasks);
        Ok(tasks)
    }

    pub(super) fn sort_by_project(tasks: &mut [(String, Task)]) {
        tasks.sort_by(|(a, x), (b, y)| (a, &x.name).cmp(&(b, &y.name)));
    }

    /// [`Self::tasks_in`] unordered, skipping finished tasks when `unfinished`.
    pub(super) fn tasks_of<G: ProjectGroup>(
        &self,
        id: i64,
        unfinished: bool,
    ) -> Result<Vec<(String, Task)>> {
        let names: HashMap<i64, String> = self
            .projects_in::<G>(id)?
            .into_iter()
            .map(|p| (p.id(), p.name))
            .collect();
        let tail = format!(
            "WHERE project_id IN (SELECT id FROM projects WHERE {} = ?1){}",
            G::MEMBER_COLUMN,
            if unfinished {
                format!(" AND {NOT_DONE}")
            } else {
                String::new()
            }
        );
        Ok(self
            .find_all::<Task>(&tail, params![id])?
            .into_iter()
            .map(|t| (names.get(&t.project_id).cloned().unwrap_or_default(), t))
            .collect())
    }

    /// Task count for group `id` without reading them (badge).
    #[cfg_attr(not(feature = "web"), allow(dead_code))]
    pub fn count_tasks_in<G: ProjectGroup>(&self, id: i64) -> Result<usize> {
        let sql = format!(
            "SELECT COUNT(*) FROM tasks WHERE project_id IN (SELECT id FROM projects WHERE {} = ?1)",
            G::MEMBER_COLUMN
        );
        let n: i64 = self
            .conn
            .prepare_cached(&sql)?
            .query_row(params![id], |row| row.get(0))?;
        Ok(n as usize)
    }
}
