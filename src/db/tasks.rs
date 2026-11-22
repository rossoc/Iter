//! Task, card, priority and tag queries.

use super::*;

impl Db {
    /// Task `id` with its project, or `None` when there is no such task.
    pub fn task_with_project(&self, id: i64) -> Result<Option<(Project, Task)>> {
        let Some(task) = self.get::<Task>(id)? else {
            return Ok(None);
        };
        let project = self.require(task.project_id, "project", "task")?;
        Ok(Some((project, task)))
    }

    /// Points `task` (row `id`, in project `from`) at project `to`; nothing is written. A task
    /// with a session cannot move: its worktree and branch live in `from`'s repo.
    pub fn move_task(&self, id: i64, task: &mut Task, from: &Project, to: &Project) -> Result<()> {
        if to.id() != from.id() {
            if self.find_session_config_by_task(id)?.is_some() {
                return Err(IterError::TaskHasSession(task.label(from).to_string()));
            }
            task.project_id = to.id();
        }
        Ok(())
    }

    /// Every task on board `id`'s projects as a [`Card`], by project then task; three queries.
    pub fn cards(&self, board_id: i64) -> Result<Vec<Card>> {
        let mut tasks = self.tasks_of::<Board>(board_id, false)?;
        Self::sort_by_project(&mut tasks);
        self.with_priorities(board_id, tasks, false)
    }

    /// [`Self::cards`] of unfinished tasks only (filtered in SQL), unordered.
    #[cfg(feature = "web")]
    pub fn cards_unfinished(&self, board_id: i64) -> Result<Vec<Card>> {
        let tasks = self.tasks_of::<Board>(board_id, true)?;
        self.with_priorities(board_id, tasks, true)
    }

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
                label: TaskLabel {
                    id: task.id(),
                    project,
                    title: task.name.clone(),
                }
                .to_string(),
                priority: priorities.get(&task.id()).copied().unwrap_or_default(),
                task,
            })
            .collect())
    }

    /// Urgent and Important tag colors, in that order; both are seeded, so a missing one is an error.
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

    /// The [`Priority`] of every task in group `id`, by task id; absent means neither flag.
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

    /// Stores `task` as row `id` and, when `priority` is some, sets its flags in the same
    /// transaction, so a board drop changes both or neither.
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

    pub fn tags_for_task(&self, task_id: i64) -> Result<Vec<Tag>> {
        self.find_all(
            "WHERE id IN (SELECT tag_id FROM task_tags WHERE task_id = ?1) ORDER BY name",
            params![task_id],
        )
    }

    /// Replaces the tags on `task_id` with exactly `tag_ids`.
    pub fn set_task_tags(&self, task_id: i64, tag_ids: &[i64]) -> Result<()> {
        self.atomically(|| self.replace_task_tags(task_id, tag_ids))
    }

    /// [`Self::set_task_tags`] for a caller with a transaction open.
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

    /// Writes `task` with exactly `tag_ids` in one transaction (insert when `id` is `None`);
    /// returns its id.
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

    /// The tasks of a project titled `title` (titles may repeat), by id.
    pub fn tasks_titled(&self, project_id: i64, title: &str) -> Result<Vec<Task>> {
        self.find_all(
            "WHERE project_id = ?1 AND name = ?2 ORDER BY id",
            params![project_id, title],
        )
    }

    /// Task count of a project, unread (tab badge).
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

    /// Every task of any of `project_ids`, by project then name; one query.
    pub fn tasks_for_projects(&self, project_ids: &[i64]) -> Result<Vec<Task>> {
        self.find_all(
            "WHERE project_id IN (SELECT value FROM json_each(?1)) ORDER BY project_id, name",
            params![json_ids(project_ids)],
        )
    }

    /// Tag names per task id, one query; untagged tasks are absent.
    #[cfg(any(test, feature = "web"))]
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

    /// Every task as a [`TaskLabel`], by project, title and id, optionally narrowed by project
    /// and statuses (empty = all): what `iter task list` prints.
    pub fn task_labels(
        &self,
        project: Option<&str>,
        statuses: &[TaskStatus],
    ) -> Result<Vec<TaskLabel>> {
        let counted = self.task_labels_counted(project, statuses)?;
        Ok(counted.into_iter().map(|(label, _)| label).collect())
    }

    /// [`task_labels`](Self::task_labels), each with how many tasks of its project share its title
    /// (counted over all statuses): one query, since completion runs it on every TAB.
    pub fn task_labels_counted(
        &self,
        project: Option<&str>,
        statuses: &[TaskStatus],
    ) -> Result<Vec<(TaskLabel, usize)>> {
        let mut sql = String::from(
            "SELECT p.name, t.name, t.id, (SELECT COUNT(*) FROM tasks s \
             WHERE s.project_id = t.project_id AND s.name = t.name) \
             FROM tasks t JOIN projects p ON p.id = t.project_id",
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
        sql.push_str(" ORDER BY p.name, t.name, t.id");

        // Anonymous `?` so the filters compose.
        let mut values: Vec<Value> = Vec::new();
        values.extend(project.map(|name| Value::Text(name.to_string())));
        values.extend(statuses.iter().map(|s| Value::Text(s.as_str().to_string())));

        self.query_all(&sql, params_from_iter(values), |row| {
            let label = TaskLabel {
                project: row.get(0)?,
                title: row.get(1)?,
                id: row.get(2)?,
            };
            Ok((label, row.get::<_, i64>(3)? as usize))
        })
    }
}
