use std::process::ExitCode;

mod check;
mod keys;
mod list;
mod output;
mod render;
mod table;

const USAGE: &str = "usage: chord render <target> --table <file>\n       chord check <target> --table <file> [--against <file>]\n       chord list --table <file> [--group <name>]\n       <target> is bash or menu";

const DIFFERS: u8 = 1;

const REFUSED: u8 = 2;

#[derive(Debug, PartialEq, Eq)]
enum Command<'a> {
    Render {
        target: &'a str,
        table: &'a str,
    },
    Check {
        target: &'a str,
        table: &'a str,
        against: Option<&'a str>,
    },
    List {
        table: &'a str,
        group: Option<&'a str>,
    },
}

fn command<'a>(words: &[&'a str]) -> Option<Command<'a>> {
    match words {
        ["render", target, "--table", table] => Some(Command::Render { target, table }),
        ["check", target, "--table", table] => Some(Command::Check {
            target,
            table,
            against: None,
        }),
        ["check", target, "--table", table, "--against", against] => Some(Command::Check {
            target,
            table,
            against: Some(against),
        }),
        ["list", "--table", table] => Some(Command::List { table, group: None }),
        ["list", "--table", table, "--group", group] => Some(Command::List {
            table,
            group: Some(group),
        }),
        _ => None,
    }
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let words: Vec<&str> = arguments.iter().map(String::as_str).collect();
    match command(&words) {
        Some(Command::Render { target, table }) => render(target, table),
        Some(Command::Check {
            target,
            table,
            against,
        }) => check(target, table, against),
        Some(Command::List { table, group }) => list(table, group),
        None => {
            eprintln!("{USAGE}");
            ExitCode::from(REFUSED)
        }
    }
}

fn render(target: &str, table_path: &str) -> ExitCode {
    let rendering = match rendering(target, table_path) {
        Ok(rendering) => rendering,
        Err(refusal) => return refuse(&refusal),
    };
    match rendering.output {
        None => {
            print!("{}", rendering.text);
            ExitCode::SUCCESS
        }
        Some(path) => match output::write_atomically(&path, &rendering.text) {
            Ok(()) => ExitCode::SUCCESS,
            Err(refusal) => refuse(&refusal),
        },
    }
}

fn check(target: &str, table_path: &str, against_path: Option<&str>) -> ExitCode {
    let rendering = match rendering(target, table_path) {
        Ok(rendering) => rendering,
        Err(refusal) => return refuse(&refusal),
    };
    let compared = match compared_path(target, against_path, rendering.output.as_deref()) {
        Ok(path) => path.to_string(),
        Err(refusal) => return refuse(&refusal),
    };
    let on_disk = match std::fs::read_to_string(&compared) {
        Ok(text) => text,
        Err(fault) => return refuse(&format!("cannot read {compared}: {fault}")),
    };
    let difference = check::difference(&on_disk, &rendering.text, &compared);
    if difference.is_empty() {
        return ExitCode::SUCCESS;
    }
    eprint!("{difference}");
    eprintln!("{}", mismatch(&compared, rendering.regenerate.as_deref()));
    ExitCode::from(DIFFERS)
}

fn list(table_path: &str, group: Option<&str>) -> ExitCode {
    let table = match parsed_table(table_path) {
        Ok(table) => table,
        Err(refusal) => return refuse(&refusal),
    };
    match list::listing(&table, group) {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(fault) => refuse(&fault.to_string()),
    }
}

fn compared_path<'a>(
    target: &str,
    against_path: Option<&'a str>,
    output: Option<&'a str>,
) -> Result<&'a str, String> {
    against_path.or(output).ok_or_else(|| {
        format!("the table names no output for {target}, so a check needs --against <file>")
    })
}

fn mismatch(against_path: &str, regenerate: Option<&str>) -> String {
    let remedy = match regenerate {
        Some(command) => format!("run `{command}`"),
        None => "render the table again".to_string(),
    };
    format!("{against_path} is not what the table renders; {remedy}")
}

struct Rendering {
    text: String,
    output: Option<String>,
    regenerate: Option<String>,
}

fn rendering(target: &str, table_path: &str) -> Result<Rendering, String> {
    let renderer = render::for_target(target)
        .ok_or_else(|| format!("chord does not render the target {target:?}"))?;
    let table = parsed_table(table_path)?;
    Ok(Rendering {
        text: renderer.render(&table).map_err(|fault| fault.to_string())?,
        output: renderer.output(&table).map(str::to_string),
        regenerate: table.render.regenerate,
    })
}

fn parsed_table(table_path: &str) -> Result<table::Table, String> {
    let text = std::fs::read_to_string(table_path)
        .map_err(|fault| format!("cannot read {table_path}: {fault}"))?;
    toml::from_str(&text).map_err(|fault| format!("{table_path}: {fault}"))
}

fn refuse(reason: &str) -> ExitCode {
    eprintln!("{reason}");
    ExitCode::from(REFUSED)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_regenerate_command_the_table_names_is_what_the_reader_is_told_to_run() {
        assert_eq!(
            mismatch("bindings.sh", Some("make bindings")),
            "bindings.sh is not what the table renders; run `make bindings`"
        );
    }

    #[test]
    fn a_table_naming_no_regenerate_command_still_says_what_is_wrong_and_names_no_tool() {
        let message = mismatch("bindings.sh", None);
        assert_eq!(
            message,
            "bindings.sh is not what the table renders; render the table again"
        );
        assert!(!message.contains('`'), "{message}");
    }
}

#[cfg(test)]
mod compared_path_tests {
    use super::*;

    #[test]
    fn an_explicit_against_path_wins_over_the_tables_output() {
        assert_eq!(
            compared_path("bash", Some("other.sh"), Some("bindings.sh")),
            Ok("other.sh")
        );
    }

    #[test]
    fn the_tables_output_is_what_a_check_compares_when_no_path_is_given() {
        assert_eq!(
            compared_path("bash", None, Some("bindings.sh")),
            Ok("bindings.sh")
        );
    }

    #[test]
    fn a_check_with_neither_a_path_nor_an_output_says_what_is_missing() {
        let refusal = compared_path("bash", None, None).expect_err("a check needs a file");
        assert!(refusal.contains("--against"), "{refusal}");
        assert!(refusal.contains("bash"), "{refusal}");
    }
}

#[cfg(test)]
mod command_tests {
    use super::*;

    #[test]
    fn a_list_with_only_a_table_lists_every_group() {
        assert_eq!(
            command(&["list", "--table", "t.toml"]),
            Some(Command::List {
                table: "t.toml",
                group: None
            })
        );
    }

    #[test]
    fn a_list_with_a_group_keeps_that_group() {
        assert_eq!(
            command(&["list", "--table", "t.toml", "--group", "git"]),
            Some(Command::List {
                table: "t.toml",
                group: Some("git")
            })
        );
    }

    #[test]
    fn a_list_with_a_missing_or_misplaced_argument_gets_the_usage() {
        for words in [
            vec!["list"],
            vec!["list", "--table"],
            vec!["list", "--table", "t.toml", "--group"],
            vec!["list", "--group", "git", "--table", "t.toml"],
            vec!["list", "bash", "--table", "t.toml"],
        ] {
            assert_eq!(command(&words), None, "{words:?}");
        }
    }

    #[test]
    fn the_usage_names_every_command() {
        assert!(USAGE.contains("chord render <target> --table <file>"));
        assert!(USAGE.contains("chord check <target> --table <file> [--against <file>]"));
        assert!(USAGE.contains("chord list --table <file> [--group <name>]"));
    }
}
