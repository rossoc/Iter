//! Task, card, priority and tag queries.

use super::*;

impl Db {
    /// Every task on board `id`'s projects as a [`Card`], ordered by
    /// project, then task. Three queries however big the board.
    pub fn cards(&self, board_id: i64) -> Result<Vec<Card>> {
        let mut tasks = self.tasks_of::<Board>(board_id, false)?;
        Self::sort_by_project(&mut tasks);
        self.with_priorities(board_id, tasks, false)
    }

    /// [`Self::cards`] of the unfinished tasks only (`status <> 'done'`
    /// is in the SQL, so finished work is never read), in no particular
    /// order: the caller sorts once, its own way.
    #[cfg(feature = "web")]
    pub fn cards_unfinished(&self, board_id: i64) -> Result<Vec<Card>> {
        let tasks = self.tasks_of::<Board>(board_id, true)?;
        self.with_priorities(board_id, tasks, true)
    }

    /// `tasks` as cards: one query for the flags of the whole board.
    pub(super) fn with_priorities(
        &self,
        board_id: i64,
        tasks: Vec<(String, Task)>,
        unfinished: bool,
    ) -> Result<Vec<Card>> {
        let priorities = self.priorities_of::<Board>(board_id, unfinished)?;
        Ok(tasks
            .into_iter()
            .map(|(project, task)| Card {
                label: task_ref(&project, &task.name),
                priority: priorities.get(&task.id()).copied().unwrap_or_default(),
                task,
            })
            .collect())
    }

    /// The colors of the Urgent and Important tags, in that order, in one
    /// query. Both tags are seeded, so a missing one is an error.
    #[cfg(feature = "web")]
    pub fn priority_colors(&self) -> Result<(String, String)> {
        let rows: HashMap<String, String> = self
            .query_all(
                "SELECT name, color FROM tags WHERE name IN (?1, ?2)",
                params![URGENT_TAG, IMPORTANT_TAG],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )?
            .into_iter()
            .collect();
        let color = |name: &str| {
            rows.get(name).cloned().ok_or_else(|| IterError::NotFound {
                kind: Tag::KIND,
                name: name.to_string(),
            })
        };
        Ok((color(URGENT_TAG)?, color(IMPORTANT_TAG)?))
    }

    /// The [`Priority`] of every task in group `id`, by task id -- one
    /// query for a listing of many, reading only that group's tag rows. A
    /// task missing from the map has neither flag.
    /// Finished tasks are left out when `unfinished`.
    pub(super) fn priorities_of<G: ProjectGroup>(
        &self,
        id: i64,
        unfinished: bool,
    ) -> Result<HashMap<i64, Priority>> {
        let rows = self.query_all(
            &format!(
                "SELECT tt.task_id, g.name FROM task_tags tt
                 JOIN tags g ON g.id = tt.tag_id
                 JOIN tasks t ON t.id = tt.task_id
                 JOIN projects p ON p.id = t.project_id
                 WHERE g.name IN (?1, ?2) AND p.{} = ?3{}",
                G::MEMBER_COLUMN,
                if unfinished {
                    format!(" AND t.{NOT_DONE}")
                } else {
                    String::new()
                }
            ),
            params![URGENT_TAG, IMPORTANT_TAG, id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )?;
        let mut out: HashMap<i64, Priority> = HashMap::new();
        for (task_id, tag) in rows {
            let priority = out.entry(task_id).or_default();
            match tag == URGENT_TAG {
                true => priority.urgent = true,
                false => priority.important = true,
            }
        }
        Ok(out)
    }

    /// Stores `task` as row `id` and, when `priority` is some, makes its
    /// flags that, in one transaction: a board's drop changes both or
    /// neither (never a task placed with the old flags).
    #[cfg(any(test, feature = "web"))]
    pub fn update_placed(&self, id: i64, task: &Task, priority: Option<Priority>) -> Result<()> {
        self.atomically(|| {
            self.update(id, task)?;
            match priority {
                Some(priority) => self.set_priority(id, priority),
                None => Ok(()),
            }
        })
    }

    /// Makes `task_id`'s flags `priority`, by adding or removing the tags.
    #[cfg(any(test, feature = "web"))]
    pub fn set_priority(&self, task_id: i64, priority: Priority) -> Result<()> {
        for (tag, on) in [
            (URGENT_TAG, priority.urgent),
            (IMPORTANT_TAG, priority.important),
        ] {
            let sql = match on {
                true => {
                    "INSERT OR IGNORE INTO task_tags (task_id, tag_id)
                     SELECT ?1, id FROM tags WHERE name = ?2"
                }
                false => {
                    "DELETE FROM task_tags
                     WHERE task_id = ?1 AND tag_id = (SELECT id FROM tags WHERE name = ?2)"
                }
            };
            self.conn
                .prepare_cached(sql)?
                .execute(params![task_id, tag])?;
        }
        Ok(())
    }

    /// Every tag attached to `task_id`, in name order.
    pub fn tags_for_task(&self, task_id: i64) -> Result<Vec<Tag>> {
        self.find_all(
            "WHERE id IN (SELECT tag_id FROM task_tags WHERE task_id = ?1) ORDER BY name",
            params![task_id],
        )
    }

    /// Replaces the tags on `task_id` with exactly `tag_ids`, in one
    /// transaction.
    pub fn set_task_tags(&self, task_id: i64, tag_ids: &[i64]) -> Result<()> {
        self.atomically(|| self.replace_task_tags(task_id, tag_ids))
    }

    /// [`Self::set_task_tags`] without its own transaction, for a caller
    /// that has one open.
    pub(super) fn replace_task_tags(&self, task_id: i64, tag_ids: &[i64]) -> Result<()> {
        self.conn
            .execute("DELETE FROM task_tags WHERE task_id = ?1", params![task_id])?;
        let mut insert = self
            .conn
            .prepare_cached("INSERT OR IGNORE INTO task_tags (task_id, tag_id) VALUES (?1, ?2)")?;
        for tag_id in tag_ids {
            insert.execute(params![task_id, tag_id])?;
        }
        Ok(())
    }

    /// Writes `task` with exactly `tag_ids`, in one transaction: a new row
    /// when `id` is `None`, else over row `id`. Returns the task's id. A new
    /// task without tags skips the tag write altogether.
    #[cfg(feature = "web")]
    pub fn save_task(&self, id: Option<i64>, task: &Task, tag_ids: &[i64]) -> Result<i64> {
        self.atomically(|| {
            let saved = match id {
                Some(id) => {
                    self.update(id, task)?;
                    id
                }
                None => self.insert(task)?,
            };
            if id.is_some() || !tag_ids.is_empty() {
                self.replace_task_tags(saved, tag_ids)?;
            }
            Ok(saved)
        })
    }

    pub fn find_task(&self, project_id: i64, task_name: &str) -> Result<Option<Task>> {
        self.find_one(
            "WHERE project_id = ?1 AND name = ?2",
            params![project_id, task_name],
        )
    }

    /// How many tasks project `project_id` holds, without reading them (a
    /// tab badge).
    #[cfg(feature = "web")]
    pub fn count_tasks_for_project(&self, project_id: i64) -> Result<usize> {
        let n: i64 = self
            .conn
            .prepare_cached("SELECT COUNT(*) FROM tasks WHERE project_id = ?1")?
            .query_row(params![project_id], |row| row.get(0))?;
        Ok(n as usize)
    }

    pub fn tasks_for_project(&self, project_id: i64) -> Result<Vec<Task>> {
        self.find_all("WHERE project_id = ?1 ORDER BY name", params![project_id])
    }

    /// Every task of any of `project_ids`, by project then name -- one query
    /// for a whole set of projects (the ids travel as one JSON array, as in
    /// `assign_projects`).
    pub fn tasks_for_projects(&self, project_ids: &[i64]) -> Result<Vec<Task>> {
        self.find_all(
            "WHERE project_id IN (SELECT value FROM json_each(?1)) ORDER BY project_id, name",
            params![json_ids(project_ids)],
        )
    }

    /// The names of the tags of each of `ids`, in one query (the ids travel
    /// as one JSON array). A task with no tag is not in the map.
    #[cfg(feature = "web")]
    pub fn tag_names_for_tasks(&self, ids: &[i64]) -> Result<HashMap<i64, Vec<String>>> {
        let rows = self.query_all(
            "SELECT tt.task_id, g.name FROM task_tags tt
             JOIN tags g ON g.id = tt.tag_id
             WHERE tt.task_id IN (SELECT value FROM json_each(?1))
             ORDER BY g.name",
            params![json_ids(ids)],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )?;
        let mut names: HashMap<i64, Vec<String>> = HashMap::new();
        for (task, name) in rows {
            names.entry(task).or_default().push(name);
        }
        Ok(names)
    }

    /// Every task as a [`task_ref`], in `<project>/<task>` order, optionally narrowed to one project and/or a set of statuses.
    /// An empty `statuses` means every status, the same way `None` for
    /// `project` means every project.
    ///
    /// One join rather than a listing plus a query per project: this runs
    /// on every TAB keypress, where the per-project round trips were the
    /// dominant cost and every task's other columns were loaded only to be
    /// dropped.
    pub fn task_refs(&self, project: Option<&str>, statuses: &[TaskStatus]) -> Result<Vec<String>> {
        let mut sql = String::from(
            "SELECT p.name, t.name FROM tasks t JOIN projects p ON p.id = t.project_id",
        );
        let mut clauses = Vec::new();
        if project.is_some() {
            clauses.push("p.name = ?".to_string());
        }
        if !statuses.is_empty() {
            let placeholders = vec!["?"; statuses.len()].join(", ");
            clauses.push(format!("t.status IN ({placeholders})"));
        }
        if !clauses.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&clauses.join(" AND "));
        }
        sql.push_str(" ORDER BY p.name, t.name");

        // Anonymous `?` bound positionally, so the filters compose without
        // each combination needing its own `query_map` arm.
        let mut values: Vec<Value> = Vec::new();
        values.extend(project.map(|name| Value::Text(name.to_string())));
        values.extend(statuses.iter().map(|s| Value::Text(s.as_str().to_string())));

        self.query_all(&sql, params_from_iter(values), |row| {
            Ok(task_ref(
                &row.get::<_, String>(0)?,
                &row.get::<_, String>(1)?,
            ))
        })
    }

    /// Every distinct task name in the database, sorted -- what `task
    /// pull/push --task` completes, which takes a bare name rather than a
    /// `<project>/<task>` pair. SQL does the dedup and the ordering.
    pub fn task_names(&self) -> Result<Vec<String>> {
        self.query_all("SELECT DISTINCT name FROM tasks ORDER BY name", [], |row| {
            row.get(0)
        })
    }
}
