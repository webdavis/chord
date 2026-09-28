use std::process::ExitCode;

mod check;
mod keys;
mod output;
mod render;
mod table;

const USAGE: &str = "usage: chord render <target> --table <file>\n       chord check <target> --table <file> [--against <file>]\n       <target> is bash or menu";

const DIFFERS: u8 = 1;

const REFUSED: u8 = 2;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let words: Vec<&str> = arguments.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["render", target, "--table", table] => render(target, table),
        ["check", target, "--table", table] => check(target, table, None),
        ["check", target, "--table", table, "--against", against] => {
            check(target, table, Some(against))
        }
        _ => {
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
    let text = std::fs::read_to_string(table_path)
        .map_err(|fault| format!("cannot read {table_path}: {fault}"))?;
    let table: table::Table =
        toml::from_str(&text).map_err(|fault| format!("{table_path}: {fault}"))?;
    Ok(Rendering {
        text: renderer.render(&table).map_err(|fault| fault.to_string())?,
        output: renderer.output(&table).map(str::to_string),
        regenerate: table.render.regenerate,
    })
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
