use super::{DockerInspect, RemoteRuntimeError, validate_upload_target};

#[test]
fn inventory_excludes_unmanaged_containers() {
    let inspect = serde_json::from_str::<DockerInspect>(
        r#"{
            "Id": "b0b0b0b0",
            "Name": "/database",
            "Config": {
                "Image": "postgres:17",
                "Labels": {"com.ignitify.managed": "false"}
            },
            "State": {"Status": "running", "Running": true}
        }"#,
    )
    .unwrap();

    assert!(inspect.into_runtime_container().unwrap().is_none());
}

#[test]
fn upload_target_rejects_parent_directory_traversal() {
    assert!(validate_upload_target("/tmp/../etc", "config").is_err());
    assert!(validate_upload_target("/tmp", "../config").is_err());
    assert!(validate_upload_target("/tmp", "config").is_ok());
}

#[test]
fn runtime_error_keeps_docker_prerequisite_diagnostic_safe() {
    assert_eq!(
        RemoteRuntimeError::DockerUnavailable.user_message(),
        "Docker is not installed or is not available to the configured SSH user."
    );
}
