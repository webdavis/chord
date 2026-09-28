use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Table {
    #[serde(default)]
    pub group: Vec<Group>,
    #[serde(default)]
    pub render: Render,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Render {
    pub regenerate: Option<String>,
    #[serde(default)]
    pub bash: BashRender,
    #[serde(default)]
    pub menu: MenuRender,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BashRender {
    pub header: Option<String>,
    pub clear_line: Option<String>,
    pub output: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MenuRender {
    pub header: Option<String>,
    pub output: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub binding: Vec<Binding>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub key: String,
    pub description: Option<String>,
    pub modes: Option<Vec<String>>,
    pub insert: Option<String>,
    pub run: Option<String>,
    pub function: Option<String>,
    pub command: Option<String>,
    #[serde(rename = "macro")]
    pub macro_body: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Action<'a> {
    Insert(&'a str),
    Run(&'a str),
    Function(&'a str),
    Command(&'a str),
    Macro(&'a str),
}

#[derive(Debug, PartialEq, Eq)]
pub enum RowFault {
    NoAction,
    SeveralActions,
}

impl std::fmt::Display for RowFault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            RowFault::NoAction => "names no action",
            RowFault::SeveralActions => "names more than one action",
        };
        formatter.write_str(message)
    }
}

impl Binding {
    pub fn action(&self) -> Result<Action<'_>, RowFault> {
        let candidates = [
            self.insert.as_deref().map(Action::Insert),
            self.run.as_deref().map(Action::Run),
            self.function.as_deref().map(Action::Function),
            self.command.as_deref().map(Action::Command),
            self.macro_body.as_deref().map(Action::Macro),
        ];
        let mut found = candidates.into_iter().flatten();
        match (found.next(), found.next()) {
            (Some(action), None) => Ok(action),
            (None, _) => Err(RowFault::NoAction),
            (Some(_), Some(_)) => Err(RowFault::SeveralActions),
        }
    }

    pub fn modes(&self) -> &[String] {
        match &self.modes {
            Some(modes) => modes,
            None => BOTH_VI_MODES.as_slice(),
        }
    }
}

static BOTH_VI_MODES: std::sync::LazyLock<[String; 2]> =
    std::sync::LazyLock::new(|| ["vi-insert".to_string(), "vi-command".to_string()]);

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Table {
        toml::from_str(text).expect("the table must parse")
    }

    #[test]
    fn a_row_without_modes_binds_in_both_vi_keymaps() {
        let table =
            parse("[[group]]\nname = \"g\"\n[[group.binding]]\nkey = \"alt-l\"\nrun = \"ls\"\n");
        assert_eq!(
            table.group[0].binding[0].modes(),
            ["vi-insert", "vi-command"]
        );
    }

    #[test]
    fn an_empty_mode_list_stays_empty() {
        let table = parse(
            "[[group]]\nname = \"g\"\n[[group.binding]]\nkey = \"tab\"\nfunction = \"f\"\nmodes = []\n",
        );
        assert!(table.group[0].binding[0].modes().is_empty());
    }

    #[test]
    fn a_row_names_exactly_one_action() {
        let table = parse(
            "[[group]]\nname = \"g\"\n[[group.binding]]\nkey = \"a\"\nrun = \"ls\"\ninsert = \"ls\"\n\
             [[group.binding]]\nkey = \"b\"\n",
        );
        assert_eq!(
            table.group[0].binding[0].action(),
            Err(RowFault::SeveralActions)
        );
        assert_eq!(table.group[0].binding[1].action(), Err(RowFault::NoAction));
    }

    #[test]
    fn a_table_naming_no_render_section_declares_nothing() {
        let table = parse("[[group]]\nname = \"g\"\n");
        assert_eq!(table.render.regenerate, None);
        assert_eq!(table.render.bash.header, None);
        assert_eq!(table.render.bash.clear_line, None);
        assert_eq!(table.render.bash.output, None);
        assert_eq!(table.render.menu.header, None);
        assert_eq!(table.render.menu.output, None);
    }

    #[test]
    fn the_render_section_carries_each_targets_own_output_settings() {
        let table = parse(
            "[render]\nregenerate = \"make bindings\"\n\
             [render.bash]\nheader = \"# mine\\n\"\nclear_line = \"\\\\C-x0\"\n\
             [render.menu]\nheader = \"# picker\\n\"\n",
        );
        assert_eq!(table.render.regenerate.as_deref(), Some("make bindings"));
        assert_eq!(table.render.bash.header.as_deref(), Some("# mine\n"));
        assert_eq!(table.render.bash.clear_line.as_deref(), Some("\\C-x0"));
        assert_eq!(table.render.menu.header.as_deref(), Some("# picker\n"));
    }

    #[test]
    fn each_target_names_the_file_it_writes() {
        let table = parse(
            "[render.bash]\noutput = \"bindings.sh\"\n[render.menu]\noutput = \"records.tsv\"\n",
        );
        assert_eq!(table.render.bash.output.as_deref(), Some("bindings.sh"));
        assert_eq!(table.render.menu.output.as_deref(), Some("records.tsv"));
    }

    #[test]
    fn a_misspelled_render_key_is_refused_rather_than_ignored() {
        let fault = toml::from_str::<Table>("[render.bash]\nheadr = \"# mine\"\n")
            .expect_err("an unknown key must be refused");
        assert!(fault.to_string().contains("headr"), "{fault}");
    }
}
