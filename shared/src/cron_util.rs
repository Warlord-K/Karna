use std::str::FromStr;

use cron::Schedule as CronSchedule;

/// Normalize a cron expression to the 6-field form the `cron` crate expects.
///
/// Karna's UI and most users write standard Unix 5-field cron
/// (`min hour dom month dow`). The `cron` crate requires a leading seconds
/// field (6 fields) or trailing year field (7). This prepends `0` so 5-field
/// input parses; 6/7-field input is passed through unchanged.
pub fn normalize(expr: &str) -> String {
    let trimmed = expr.trim();
    if trimmed.split_whitespace().count() == 5 {
        format!("0 {trimmed}")
    } else {
        trimmed.to_string()
    }
}

/// Validate that a cron expression parses after normalization.
/// Returns the underlying parser error message on failure.
pub fn validate(expr: &str) -> Result<(), String> {
    let normalized = normalize(expr);
    CronSchedule::from_str(&normalized).map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_field_patterns_parse_after_normalization() {
        // Patterns the user asked about
        assert!(validate("0 9 * * 1,4").is_ok());
        assert!(validate("0 */5 * * *").is_ok());
        // Patterns shipped in the frontend presets
        for preset in [
            "* * * * *",
            "*/5 * * * *",
            "*/15 * * * *",
            "*/30 * * * *",
            "0 * * * *",
            "0 */2 * * *",
            "0 */4 * * *",
            "0 */6 * * *",
            "0 */12 * * *",
            "0 0 * * *",
            "0 9 * * *",
            "0 9 * * 1",
            "0 9 * * 1-5",
        ] {
            assert!(validate(preset).is_ok(), "preset failed: {preset}");
        }
    }

    #[test]
    fn six_and_seven_field_patterns_pass_through() {
        assert!(validate("0 0 9 * * Mon,Thu").is_ok());
        assert!(validate("0 0 9 * * Mon,Thu *").is_ok());
    }

    #[test]
    fn whitespace_is_trimmed() {
        assert_eq!(normalize("  0 9 * * 1,4  "), "0 0 9 * * 1,4");
    }

    #[test]
    fn invalid_expressions_are_rejected() {
        assert!(validate("not a cron").is_err());
        assert!(validate("0 99 * * *").is_err());
    }
}
