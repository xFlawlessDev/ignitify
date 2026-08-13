use super::{
    is_authentication_failure, normalize_private_key, public_key_material, ssh_failure_error,
    ssh_keygen_failure_error, validate_private_key,
};
use crate::error::ApiError;

#[test]
fn rejects_private_key_without_matching_footer() {
    let result =
        validate_private_key("-----BEGIN OPENSSH PRIVATE KEY-----\nprivate-key".to_owned());

    assert!(matches!(
        result,
        Err(ApiError::BadRequest("SSH private key is invalid"))
    ));
}

#[test]
fn normalizes_private_key_line_endings_and_body_whitespace() {
    assert_eq!(
        normalize_private_key(
            "-----BEGIN OPENSSH PRIVATE KEY-----\r\nabc \r\n def\r\n-----END OPENSSH PRIVATE KEY-----"
                .to_owned()
        ),
        "-----BEGIN OPENSSH PRIVATE KEY-----\nabcdef\n-----END OPENSSH PRIVATE KEY-----\n"
    );
}

#[test]
fn maps_ssh_authentication_failure_without_exposing_stderr() {
    let error = ssh_failure_error(b"user@host: Permission denied (publickey).");

    assert!(matches!(
        error,
        ApiError::RemoteServerCheckFailedWithReason(
            "SSH authentication failed. Install the matching public key in ~/.ssh/authorized_keys."
        )
    ));
}

#[test]
fn identifies_only_ssh_authentication_failures_for_alerting() {
    assert!(is_authentication_failure(b"Permission denied (publickey)."));
    assert!(!is_authentication_failure(b"Host key verification failed."));
}

#[test]
fn public_key_material_ignores_the_optional_comment() {
    assert_eq!(
        public_key_material("ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIExample deploy@host"),
        Some(("ssh-ed25519", "AAAAC3NzaC1lZDI1NTE5AAAAIExample"))
    );
}

#[test]
fn maps_encrypted_private_key_diagnostic_without_exposing_stderr() {
    let error = ssh_keygen_failure_error(
        b"incorrect passphrase supplied to decrypt private key",
        b"-----BEGIN OPENSSH PRIVATE KEY-----\nkey\n-----END OPENSSH PRIVATE KEY-----",
    );

    match error {
        ApiError::RemoteServerCheckFailedWithDiagnostic(message) => {
            assert!(message.contains("SSH private key has a passphrase"));
            assert!(message.contains("Received "));
            assert!(message.contains("BEGIN marker: true"));
            assert!(!message.contains("incorrect passphrase"));
        }
        _ => panic!("expected diagnostic error"),
    }
}
