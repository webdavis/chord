use crate::table::{RowFault, Table};

#[derive(Debug, PartialEq, Eq)]
pub enum ListFault {
    UnknownGroup(String),
    Row { key: String, fault: RowFault },
}

impl std::fmt::Display for ListFault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListFault::UnknownGroup(name) => write!(formatter, "error[unknown-group]: {name}"),
            ListFault::Row { key, fault } => write!(formatter, "binding {key:?}: {fault}"),
        }
    }
}

pub fn listing(table: &Table, group: Option<&str>) -> Result<String, ListFault> {
    if let Some(name) = unknown_group(table, group) {
        return Err(ListFault::UnknownGroup(name.to_string()));
    }
    Ok(aligned(&rows(table, group)?))
}

fn unknown_group<'a>(table: &Table, group: Option<&'a str>) -> Option<&'a str> {
    group.filter(|name| !table.group.iter().any(|candidate| candidate.name == *name))
}

struct Row {
    key: String,
    group: String,
    description: String,
    action: String,
}

fn rows(table: &Table, group: Option<&str>) -> Result<Vec<Row>, ListFault> {
    table
        .group
        .iter()
        .filter(|candidate| group.is_none_or(|name| candidate.name == name))
        .flat_map(|candidate| {
            candidate
                .binding
                .iter()
                .map(move |binding| (candidate, binding))
        })
        .map(|(candidate, binding)| {
            let action = binding.action().map_err(|fault| ListFault::Row {
                key: binding.key.clone(),
                fault,
            })?;
            Ok(Row {
                key: binding.key.clone(),
                group: candidate.name.clone(),
                description: single_line(binding.description.as_deref().unwrap_or_default()),
                action: single_line(action.body()),
            })
        })
        .collect()
}

fn single_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<&str>>().join(" ")
}

fn aligned(rows: &[Row]) -> String {
    let key_width = width(rows, |row| &row.key);
    let group_width = width(rows, |row| &row.group);
    let description_width = width(rows, |row| &row.description);
    rows.iter()
        .map(|row| {
            let line = format!(
                "{:key_width$}  {:group_width$}  {:description_width$}  {}",
                row.key, row.group, row.description, row.action
            );
            format!("{}\n", line.trim_end())
        })
        .collect()
}

fn width(rows: &[Row], field: fn(&Row) -> &str) -> usize {
    rows.iter()
        .map(|row| field(row).chars().count())
        .max()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO_GROUPS: &str = r#"
[[group]]
name = "git"

[[group.binding]]
key = "ctrl-g a a"
description = "Stage a path."
insert = "git add "

[[group.binding]]
key = "ctrl-g c"
run = "git commit"

[[group]]
name = "line"

[[group.binding]]
key = "ctrl-a"
description = "Go to the start of the line."
command = "beginning-of-line"

[[group.binding]]
key = "tab"
modes = []
function = "menu-complete"
"#;

    fn parse(text: &str) -> Table {
        toml::from_str(text).expect("the table must parse")
    }

    #[test]
    fn every_binding_is_one_aligned_line_in_table_order() {
        assert_eq!(
            listing(&parse(TWO_GROUPS), None).expect("the table must list"),
            "ctrl-g a a  git   Stage a path.                 git add\n\
             ctrl-g c    git                                 git commit\n\
             ctrl-a      line  Go to the start of the line.  beginning-of-line\n\
             tab         line                                menu-complete\n"
        );
    }

    #[test]
    fn a_group_filter_keeps_only_that_groups_bindings_and_aligns_them_alone() {
        assert_eq!(
            listing(&parse(TWO_GROUPS), Some("line")).expect("the group must list"),
            "ctrl-a  line  Go to the start of the line.  beginning-of-line\n\
             tab     line                                menu-complete\n"
        );
    }

    #[test]
    fn a_group_the_table_does_not_have_is_refused_by_name() {
        let fault = listing(&parse(TWO_GROUPS), Some("gti")).expect_err("an unknown group");
        assert_eq!(fault, ListFault::UnknownGroup("gti".to_string()));
        assert_eq!(fault.to_string(), "error[unknown-group]: gti");
    }

    #[test]
    fn a_long_key_is_never_cut_short() {
        let rendered = listing(
            &parse(
                "[[group]]\nname = \"g\"\n\
                 [[group.binding]]\nkey = \"ctrl-x ctrl-g alt-z esc [ Z enter\"\nrun = \"ls\"\n\
                 [[group.binding]]\nkey = \"a\"\nrun = \"pwd\"\n",
            ),
            None,
        )
        .expect("the table must list");
        assert!(
            rendered.starts_with("ctrl-x ctrl-g alt-z esc [ Z enter  g"),
            "{rendered}"
        );
    }

    #[test]
    fn a_multi_line_description_becomes_one_line() {
        let rendered = listing(
            &parse(
                "[[group]]\nname = \"g\"\n[[group.binding]]\nkey = \"a\"\n\
                 description = \"First line.\\n\\nSecond line.\"\nrun = \"ls\"\n",
            ),
            None,
        )
        .expect("the table must list");
        assert_eq!(rendered, "a  g  First line. Second line.  ls\n");
    }

    #[test]
    fn a_table_with_no_bindings_lists_nothing() {
        assert_eq!(
            listing(&parse("[[group]]\nname = \"g\"\n"), None).expect("an empty group must list"),
            ""
        );
    }

    #[test]
    fn a_row_naming_no_action_is_refused_by_key() {
        let fault = listing(
            &parse("[[group]]\nname = \"g\"\n[[group.binding]]\nkey = \"a\"\n"),
            None,
        )
        .expect_err("a row without an action must be refused");
        assert_eq!(fault.to_string(), "binding \"a\": names no action");
    }
}
