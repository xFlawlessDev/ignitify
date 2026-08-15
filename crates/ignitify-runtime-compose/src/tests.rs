use super::{Error, service_statuses};
use crate::{
    policy::{MAX_DEPTH, is_compose_image, preflight_yaml, validate_canonical},
    render::{MANAGED_LABEL, PROXY_NETWORK},
};
use serde_json::json;

use {
    ignitify_control_plane::RuntimeDeployment,
    ignitify_domain::{DeploymentId, ServiceId, ServiceSpec},
};

#[cfg(unix)]
use {
    ignitify_control_plane::ImageRuntime,
    std::{fs, os::unix::fs::PermissionsExt},
};

fn deployment() -> RuntimeDeployment {
    RuntimeDeployment {
            id: DeploymentId::new("00000000-0000-0000-0000-000000000001").unwrap(),
            service_id: ServiceId::new("00000000-0000-0000-0000-000000000002").unwrap(),
            generation: 1,
            spec: ServiceSpec::compose(
                "services:\n  web:\n    image: nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n",
                "web",
                Some(8080),
            )
            .unwrap(),
            local_image_id: None,
            deployment_destination_id: None,
        }
}

#[cfg(unix)]
fn fake_docker(temp: &tempfile::TempDir) -> std::path::PathBuf {
    let executable = temp.path().join("fake-docker");
    fs::write(
            &executable,
            "#!/bin/sh\nprintf 'cwd=<%s> ' \"$PWD\" >> \"$0.log\"\nprintf 'args=' >> \"$0.log\"\nfor argument in \"$@\"; do printf '<%s>' \"$argument\" >> \"$0.log\"; done\nprintf '\\n' >> \"$0.log\"\nenv | sort > \"$0.env\"\nfor argument in \"$@\"; do\n  if [ \"$argument\" = logs ]; then\n    printf 'stdout log\\n'\n    printf 'stderr log\\n' >&2\n    exit 0\n  fi\n  if [ \"$argument\" = up ] && [ -f \"$0.fail-up\" ]; then\n    printf 'up failed\\n' >&2\n    exit 1\n  fi\ndone\nfor argument in \"$@\"; do\n  if [ \"$argument\" = config ]; then\n    printf '{\"services\":{\"web\":{\"image\":\"nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}}}'\n    exit 0\n  fi\ndone\n",
        )
        .unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    executable
}

#[cfg(unix)]
fn cleanup_docker(temp: &tempfile::TempDir) -> std::path::PathBuf {
    let executable = temp.path().join("cleanup-docker");
    fs::write(
        &executable,
        r#"#!/bin/sh
printf 'args=' >> "$0.log"
for argument in "$@"; do printf '<%s>' "$argument" >> "$0.log"; done
printf '\n' >> "$0.log"
for argument in "$@"; do
  if [ "$argument" = down ]; then
    printf 'down failed\n' >&2
    exit 1
  fi
done
if [ "$1" = ps ]; then
  if [ -f "$0.owned-containers" ]; then cat "$0.owned-containers"; fi
  exit 0
fi
if [ "$1" = container ] && [ "$2" = rm ]; then
  rm -f "$0.owned-containers"
  exit 0
fi
"#,
    )
    .unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    executable
}

#[test]
fn compose_command_uses_fixed_argument_order() {
    let args = super::compose_args(
        std::path::Path::new("/var/lib/ignitify/compose/service/1"),
        "ignitify-service-g1",
    );
    let stage = std::path::Path::new("/var/lib/ignitify/compose/service/1");
    assert_eq!(
        args,
        [
            "compose".to_owned(),
            "--project-directory".to_owned(),
            stage.display().to_string(),
            "--project-name".to_owned(),
            "ignitify-service-g1".to_owned(),
            "--file".to_owned(),
            stage.join("compose.yaml").display().to_string(),
            "--env-file".to_owned(),
            stage.join("ignitify.env").display().to_string(),
        ]
    );
}

#[test]
fn service_statuses_ignores_stopped_companion_services() {
    let records = vec![
        json!({ "Service": "web", "State": "running" }),
        json!({ "Service": "migration", "State": "exited" }),
    ];

    assert_eq!(
        service_statuses(&records, "web"),
        vec!["running".to_owned()]
    );
}

#[test]
fn override_without_routes_keeps_compose_service_off_proxy_network() {
    let content = super::render_override(&deployment(), &[], vec![]).unwrap();

    assert!(!content.contains(PROXY_NETWORK));
    assert!(content.contains(MANAGED_LABEL));
}

#[test]
fn override_uses_a_stable_volume_name_per_service() {
    let deployment = deployment();
    let canonical = json!({ "volumes": { "data": {} } });
    let content = super::render_override(
        &deployment,
        &[],
        super::canonical_volume_names(&deployment, &canonical),
    )
    .unwrap();

    assert!(content.contains("\"data\":"));
    assert!(content.contains("name: ignitify-00000000-0000-0000-0000-000000000002-"));
}

#[test]
fn override_keeps_route_labels_inside_exposed_service() {
    let deployment = deployment();
    let route = ignitify_control_plane::IngressRoute {
        labels: std::collections::BTreeMap::from([(
            "traefik.enable".to_owned(),
            "true".to_owned(),
        )]),
        network: PROXY_NETWORK.to_owned(),
    };
    let content = super::render_override(&deployment, &[route], vec![]).unwrap();

    assert!(content.contains("    labels:\n      com.ignitify.managed: \"true\""));
    assert!(content.contains("      traefik.enable: \"true\""));
    assert!(content.contains("    networks:\n      - ignitify-proxy"));
    assert!(content.find("    labels:").unwrap() < content.find("networks:").unwrap());
}

#[cfg(unix)]
#[tokio::test]
async fn fake_docker_enforces_fixed_commands_and_cleans_failed_stages() {
    let temp = tempfile::tempdir().unwrap();
    let docker = fake_docker(&temp);
    let root = temp.path().join("stages");
    let runtime = super::ComposeRuntime::new(&docker, &root).unwrap();
    let deployment = deployment();
    let stage = root.join(deployment.service_id.as_str()).join("1");
    let project = "ignitify-00000000-0000-0000-0000-000000000002-g1";

    let runtime_ref = runtime
        .start(&deployment, vec!["TOKEN=value".to_owned()])
        .await
        .unwrap();
    assert_eq!(runtime_ref, project);
    assert_eq!(
        fs::read_to_string(stage.join("ignitify.env")).unwrap(),
        "TOKEN=value"
    );
    assert_eq!(
        fs::read_to_string(docker.with_extension("env"))
            .unwrap()
            .lines()
            .collect::<Vec<_>>(),
        ["PATH=/usr/bin:/bin", &format!("PWD={}", stage.display()),]
    );

    let calls = fs::read_to_string(docker.with_extension("log")).unwrap();
    let base = format!(
        "cwd=<{stage}> args=<compose><--project-directory><{stage}><--project-name><{project}><--file><{stage}/compose.yaml><--env-file><{stage}/ignitify.env>",
        stage = stage.display(),
    );
    assert_eq!(
        calls.lines().collect::<Vec<_>>(),
        [
            format!("{base}<config><--format><json>"),
            format!(
                "{base}<--file><{}/ignitify.override.yaml><config><--format><json>",
                stage.display()
            ),
            format!(
                "{base}<--file><{}/ignitify.override.yaml><up><--detach><--no-build><--remove-orphans>",
                stage.display()
            ),
        ],
    );

    let logs = runtime.logs(project, 0).await.unwrap();
    assert_eq!(
        logs.into_iter()
            .map(|log| (log.stream, log.line))
            .collect::<Vec<_>>(),
        [
            ("stdout".to_owned(), "stdout log".to_owned()),
            ("stderr".to_owned(), "stderr log".to_owned())
        ],
    );

    let failed_deployment = RuntimeDeployment {
        generation: 2,
        ..deployment
    };
    let failed_stage = root
        .join(failed_deployment.service_id.as_str())
        .join(failed_deployment.generation.to_string());
    fs::write(docker.with_extension("fail-up"), "").unwrap();
    assert!(runtime.start(&failed_deployment, vec![]).await.is_err());
    assert!(!failed_stage.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn stop_falls_back_to_exact_owned_container_cleanup() {
    let temp = tempfile::tempdir().unwrap();
    let docker = cleanup_docker(&temp);
    let root = temp.path().join("stages");
    let runtime = super::ComposeRuntime::new(&docker, &root).unwrap();
    let deployment = deployment();
    let stage = root.join(deployment.service_id.as_str()).join("1");
    fs::create_dir_all(&stage).unwrap();
    let runtime_ref = format!(
        "ignitify-{}-g{}",
        deployment.service_id, deployment.generation
    );
    let container_id = "a".repeat(64);
    fs::write(
        docker.with_extension("owned-containers"),
        format!("{container_id}\n"),
    )
    .unwrap();

    assert!(
        runtime
            .stop(
                &runtime_ref,
                deployment.service_id.as_str(),
                deployment.generation,
            )
            .await
            .unwrap()
    );
    assert!(!stage.exists());
    assert!(!docker.with_extension("owned-containers").exists());

    let calls = fs::read_to_string(docker.with_extension("log")).unwrap();
    assert!(calls.contains("<down><--remove-orphans>"));
    assert!(calls.contains(&format!(
        "<ps><--all><--quiet><--filter><label=com.ignitify.managed=true><--filter><label=com.ignitify.service-id={}><--filter><label=com.ignitify.generation=1>",
        deployment.service_id
    )));
    assert!(calls.contains(&format!("<container><rm><--force><{container_id}>")));
}

#[cfg(unix)]
#[tokio::test]
async fn stop_removes_owned_containers_when_compose_stage_is_missing() {
    let temp = tempfile::tempdir().unwrap();
    let docker = cleanup_docker(&temp);
    let root = temp.path().join("stages");
    let runtime = super::ComposeRuntime::new(&docker, &root).unwrap();
    let deployment = deployment();
    let runtime_ref = format!(
        "ignitify-{}-g{}",
        deployment.service_id, deployment.generation
    );
    let container_id = "b".repeat(64);
    fs::write(
        docker.with_extension("owned-containers"),
        format!("{container_id}\n"),
    )
    .unwrap();

    assert!(
        runtime
            .stop(
                &runtime_ref,
                deployment.service_id.as_str(),
                deployment.generation,
            )
            .await
            .unwrap()
    );
    assert!(!docker.with_extension("owned-containers").exists());

    let calls = fs::read_to_string(docker.with_extension("log")).unwrap();
    assert!(!calls.contains("<down><--remove-orphans>"));
    assert!(calls.contains(&format!("<container><rm><--force><{container_id}>")));
}

#[cfg(unix)]
#[tokio::test]
async fn deploys_private_compose_service_with_generated_labels_when_opted_in() {
    if std::env::var("IGNITIFY_DOCKER_TEST").as_deref() != Ok("1") {
        return;
    }
    let docker = std::path::PathBuf::from("/usr/bin/docker");
    if !docker.exists() {
        return;
    }
    let temp = tempfile::tempdir().unwrap();
    let runtime = super::ComposeRuntime::new(&docker, temp.path()).unwrap();
    let service_id = ServiceId::new(uuid::Uuid::new_v4().to_string()).unwrap();
    let deployment = RuntimeDeployment {
            id: DeploymentId::new(uuid::Uuid::new_v4().to_string()).unwrap(),
            service_id,
            generation: 1,
            spec: ServiceSpec::compose(
                "services:\n  web:\n    image: caddy:2.11.4-alpine@sha256:98eb57d882ccd5213d1688764db10c1ca2c58a1ca3a6717a3411ad798f7a423a\n",
                "web",
                Some(80),
            )
            .unwrap(),
            local_image_id: None,
            deployment_destination_id: None,
        };
    let network = PROXY_NETWORK;
    let created_network = !tokio::process::Command::new(&docker)
        .args(["network", "inspect", network])
        .output()
        .await
        .unwrap()
        .status
        .success();
    if created_network {
        let output = tokio::process::Command::new(&docker)
            .args(["network", "create", network])
            .output()
            .await
            .unwrap();
        assert!(output.status.success());
    }
    let runtime_ref = runtime.start(&deployment, vec![]).await.unwrap();
    let labels = std::collections::BTreeMap::from([
        ("traefik.enable".to_owned(), "true".to_owned()),
        (
            "traefik.http.routers.ignitify-test.rule".to_owned(),
            "Host(`compose-test.example.com`)".to_owned(),
        ),
        (
            "traefik.http.services.ignitify-test.loadbalancer.server.port".to_owned(),
            "80".to_owned(),
        ),
    ]);
    let result = async {
        runtime
            .reconcile_routes(
                &deployment,
                &runtime_ref,
                vec![],
                vec![ignitify_control_plane::IngressRoute {
                    labels,
                    network: network.to_owned(),
                }],
            )
            .await
            .map_err(|error| error.to_string())?;
        let inspect = tokio::process::Command::new(&docker)
            .args(["inspect", &runtime_ref])
            .output()
            .await
            .map_err(|error| error.to_string())?;
        if !inspect.status.success() {
            return Err("could not inspect Compose container".to_owned());
        }
        let inspect = String::from_utf8_lossy(&inspect.stdout);
        if !inspect.contains("com.ignitify.managed")
            || !inspect.contains("traefik.enable")
            || !inspect.contains("ignitify-proxy")
            || inspect.contains("\"PortBindings\": {")
        {
            return Err(
                "Compose runtime did not preserve private managed ingress contract".to_owned(),
            );
        }
        Ok(())
    }
    .await;
    let stop = runtime
        .stop(
            &runtime_ref,
            deployment.service_id.as_str(),
            deployment.generation,
        )
        .await;
    if created_network {
        let _ = tokio::process::Command::new(&docker)
            .args(["network", "rm", network])
            .output()
            .await;
    }
    assert!(stop.unwrap());
    assert!(result.is_ok(), "{}", result.unwrap_err());
}

#[test]
fn compose_policy_error_stays_typed() {
    assert!(matches!(
        super::control_error(super::Error::Policy("invalid YAML")),
        ignitify_control_plane::Error::Policy("invalid YAML")
    ));
}

#[test]
fn accepts_safe_digest_compose() {
    let yaml = "services:\n  web:\n    image: nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n    volumes:\n      - data:/data\nvolumes:\n  data: {}\n";
    preflight_yaml(yaml).unwrap();
    validate_canonical(
            &json!({
                "services": {
                    "web": {
                        "image": "nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                        "volumes": [{
                            "type": "volume",
                            "source": "data",
                            "target": "/data",
                            "volume": {}
                        }]
                    }
                },
                "volumes": { "data": { "name": "ignitify_test_data" } }
            }),
            false,
        )
        .unwrap();
}

#[test]
fn rejects_compose_images_with_mutable_tags() {
    let yaml = "services:\n  router:\n    image: decolua/9router:0.5.40\n  headroom:\n    image: ghcr.io/chopratejas/headroom:0.6.7\n";
    assert!(preflight_yaml(yaml).is_err());
    assert!(!is_compose_image("decolua/9router"));
}

#[test]
fn reports_the_specific_rejected_compose_field() {
    assert!(matches!(
        preflight_yaml(
            "services:\n  web:\n    image: nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n    ports:\n      - 8080:8080\n"
        ),
        Err(Error::Policy("Compose host ports are forbidden"))
    ));
    assert!(matches!(
        validate_canonical(
            &json!({
                "services": {
                    "web": {
                        "image": "nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                        "ports": ["8080:8080"],
                    }
                }
            }),
            false,
        ),
        Err(Error::Policy("Compose host ports are forbidden"))
    ));
}

#[test]
fn generated_labels_are_allowed_only_after_platform_override() {
    let value = json!({
        "services": {
            "web": {
                "image": "nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "labels": { "traefik.enable": "true" }
            }
        }
    });

    assert!(validate_canonical(&value, false).is_err());
    assert!(validate_canonical(&value, true).is_ok());
}

#[test]
fn rejects_host_escape_fields() {
    for key in [
        "build",
        "ports",
        "privileged",
        "devices",
        "network_mode",
        "pid",
        "ipc",
        "uts",
        "cap_add",
        "gpus",
        "volumes_from",
    ] {
        let value = json!({"services":{"web":{"image":"nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", key: []}}});
        assert!(validate_canonical(&value, false).is_err(), "{key}");
    }
    assert!(validate_canonical(&json!({"services":{"web":{"image":"nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "labels":{"traefik.enable":"true"}}}}), false).is_err());
    assert!(validate_canonical(&json!({"services":{"web":{"image":"nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "volumes":["/tmp:/data"]}}}), false).is_err());
    assert!(validate_canonical(&json!({"services":{"web":{"image":"nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}, "volumes":{"data":{"external":true}}}), false).is_err());
    assert!(preflight_yaml("services:\n  web:\n    image: nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n    volumes:\n      - data:/data\nvolumes:\n  data:\n    name: tenant-data\n").is_err());
}

#[test]
fn fixture_safe_documents_pass_preflight() {
    for fixture in [
        include_str!("../tests/fixtures/safe-web.yaml"),
        include_str!("../tests/fixtures/safe-volume.yaml"),
    ] {
        preflight_yaml(fixture).unwrap();
    }
}

#[test]
fn fixture_forbidden_yaml_document_fails_preflight() {
    assert!(
        preflight_yaml(include_str!(
            "../tests/fixtures/rejected-yaml-mechanisms.yaml"
        ))
        .is_err()
    );
}

#[test]
fn rejects_aliases_and_deep_documents() {
    assert!(
            preflight_yaml("services:\n  web: &web\n    image: nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n").is_err()
        );
    let mut value = String::from("x:");
    for _ in 0..=MAX_DEPTH {
        value.push_str("\n  x:");
    }
    value.push_str(" true\n");
    assert!(preflight_yaml(&value).is_err());
}
