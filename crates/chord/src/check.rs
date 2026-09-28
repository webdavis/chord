use similar::TextDiff;

pub fn difference(on_disk: &str, rendered: &str, path: &str) -> String {
    if on_disk == rendered {
        return String::new();
    }
    TextDiff::from_lines(on_disk, rendered)
        .unified_diff()
        .header(path, "chord render")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_text_has_no_difference() {
        assert!(difference("a\nb\n", "a\nb\n", "f").is_empty());
    }

    #[test]
    fn unequal_text_reports_both_sides() {
        let diff = difference("a\nb\n", "a\nc\n", "f");
        assert!(diff.contains("-b"), "{diff}");
        assert!(diff.contains("+c"), "{diff}");
    }
}
