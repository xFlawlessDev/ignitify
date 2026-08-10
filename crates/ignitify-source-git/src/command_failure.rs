use std::fmt;

#[derive(Debug)]
pub(super) struct GitCheckoutFailure(String);

impl GitCheckoutFailure {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for GitCheckoutFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

pub(super) fn is_git_action(action: &str) -> bool {
    matches!(
        action,
        "git checkout" | "git revision fetch" | "git revision checkout"
    )
}

pub(super) fn classify_git_failure(action: &str, stderr: &[u8]) -> GitCheckoutFailure {
    let stderr = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    if stderr.contains("remote branch") && stderr.contains("not found")
        || stderr.contains("could not find remote branch")
    {
        return GitCheckoutFailure::new("the configured branch does not exist in the repository");
    }
    if action == "git revision fetch"
        && (stderr.contains("couldn't find remote ref")
            || stderr.contains("could not find remote ref")
            || stderr.contains("invalid refspec"))
    {
        return GitCheckoutFailure::new(
            "the selected source revision is not available in the repository",
        );
    }
    if stderr.contains("repository not found")
        || stderr.contains("does not appear to be a git repository")
    {
        return GitCheckoutFailure::new(
            "the repository is unavailable to the configured provider or its path is incorrect",
        );
    }
    if stderr.contains("authentication failed")
        || stderr.contains("could not read username")
        || stderr.contains("could not read password")
        || stderr.contains("terminal prompts disabled")
        || stderr.contains("http basic: access denied")
        || stderr.contains("invalid username or password")
        || stderr.contains("invalid username or token")
        || stderr.contains("permission to ")
        || stderr.contains("access denied")
        || stderr.contains("requested url returned error: 401")
        || stderr.contains("requested url returned error: 403")
    {
        return GitCheckoutFailure::new(
            "the provider rejected repository credentials; verify GitHub App installation and Contents: Read permission",
        );
    }
    if stderr.contains("could not resolve host")
        || stderr.contains("failed to connect")
        || stderr.contains("connection timed out")
        || stderr.contains("network is unreachable")
        || stderr.contains("connection was reset")
        || stderr.contains("ssl certificate problem")
    {
        return GitCheckoutFailure::new("the control-plane host could not reach the Git provider");
    }
    GitCheckoutFailure::new(unclassified_git_failure(stderr.as_str()))
}

fn unclassified_git_failure(stderr: &str) -> String {
    let Some(line) = stderr
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| line.starts_with("fatal:"))
    else {
        return "Git did not return a recognized checkout diagnostic".to_owned();
    };
    if line.to_ascii_lowercase().contains("authorization:") {
        return "Git returned an unclassified authentication diagnostic".to_owned();
    }
    let summary = redact_quoted_values(line);
    let summary = summary
        .chars()
        .filter(|character| !character.is_control())
        .take(240)
        .collect::<String>();
    format!("Git reported: {summary}")
}

fn redact_quoted_values(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut quote = None;
    for character in value.chars() {
        if let Some(current_quote) = quote {
            if character == current_quote {
                quote = None;
            }
            continue;
        }
        if matches!(character, '\'' | '"') {
            quote = Some(character);
            output.push_str("[redacted]");
        } else {
            output.push(character);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::classify_git_failure;

    #[test]
    fn checkout_failure_does_not_expose_git_stderr() {
        let failure = classify_git_failure(
            "git checkout",
            b"fatal: Authentication failed for 'https://github.com/example/private.git'",
        );
        assert_eq!(
            failure.to_string(),
            "the provider rejected repository credentials; verify GitHub App installation and Contents: Read permission"
        );
        assert!(!failure.to_string().contains("github.com"));
    }

    #[test]
    fn checkout_failure_identifies_a_missing_branch() {
        assert_eq!(
            classify_git_failure(
                "git checkout",
                b"fatal: Remote branch staging not found in upstream origin",
            )
            .to_string(),
            "the configured branch does not exist in the repository"
        );
    }

    #[test]
    fn checkout_failure_identifies_github_http_access_denial() {
        assert_eq!(
            classify_git_failure(
                "git checkout",
                b"fatal: unable to access 'https://github.com/owner/repository.git/': The requested URL returned error: 403",
            )
            .to_string(),
            "the provider rejected repository credentials; verify GitHub App installation and Contents: Read permission"
        );
    }

    #[test]
    fn unclassified_checkout_failure_redacts_quoted_values() {
        assert_eq!(
            classify_git_failure(
                "git checkout",
                b"fatal: bad config line 2 in file '/tmp/deployment.gitconfig'",
            )
            .to_string(),
            "Git reported: fatal: bad config line 2 in file [redacted]"
        );
    }
}
