use std::collections::BTreeMap;

use ignitify_control_plane::{Ingress, IngressRoute, Result as ControlResult};
use ignitify_domain::{DomainId, DomainName, ServiceId};
use thiserror::Error;

pub const PROXY_NETWORK: &str = "ignitify-proxy";
pub const ENTRYPOINT: &str = "websecure";
pub const CERT_RESOLVER: &str = "le";

#[derive(Debug, Clone, Default)]
pub struct TraefikIngress;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum Error {
    #[error("internal route port must be between 1 and 65535")]
    InvalidPort,
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn render_route(
    domain_id: &DomainId,
    hostname: &DomainName,
    port: u32,
) -> Result<IngressRoute> {
    if !(1..=65_535).contains(&port) {
        return Err(Error::InvalidPort);
    }
    let name = format!("ignitify-{}", domain_id);
    let router = format!("traefik.http.routers.{name}");
    let service = format!("traefik.http.services.{name}");
    let labels = BTreeMap::from([
        ("traefik.enable".to_owned(), "true".to_owned()),
        (format!("{router}.rule"), format!("Host(`{hostname}`)")),
        (format!("{router}.entrypoints"), ENTRYPOINT.to_owned()),
        (format!("{router}.tls"), "true".to_owned()),
        (
            format!("{router}.tls.certresolver"),
            CERT_RESOLVER.to_owned(),
        ),
        (
            format!("{service}.loadbalancer.server.port"),
            port.to_string(),
        ),
    ]);
    Ok(IngressRoute {
        labels,
        network: PROXY_NETWORK.to_owned(),
    })
}

impl Ingress for TraefikIngress {
    fn route(
        &self,
        _service_id: &ServiceId,
        domain_id: &DomainId,
        hostname: &DomainName,
        port: u32,
    ) -> ControlResult<IngressRoute> {
        render_route(domain_id, hostname, port).map_err(|_| ignitify_control_plane::Error::Runtime)
    }
}

#[cfg(test)]
mod tests {
    use ignitify_domain::{DomainId, DomainName};

    use super::{CERT_RESOLVER, ENTRYPOINT, PROXY_NETWORK, render_route};

    #[test]
    fn route_labels_are_platform_owned_and_fixed() {
        let domain_id = DomainId::new("00000000-0000-4000-8000-000000000001").unwrap();
        let hostname = DomainName::new("app.example.com").unwrap();
        let route = render_route(&domain_id, &hostname, 8080).unwrap();
        assert_eq!(route.network, PROXY_NETWORK);
        assert_eq!(
            route.labels["traefik.http.routers.ignitify-00000000-0000-4000-8000-000000000001.entrypoints"],
            ENTRYPOINT
        );
        assert_eq!(
            route.labels["traefik.http.routers.ignitify-00000000-0000-4000-8000-000000000001.tls.certresolver"],
            CERT_RESOLVER
        );
        assert_eq!(route.labels.len(), 6);
        assert!(route.labels.keys().all(|key| key.starts_with("traefik.")));
    }

    #[test]
    fn route_rejects_invalid_port() {
        let domain_id = DomainId::new("00000000-0000-4000-8000-000000000001").unwrap();
        let hostname = DomainName::new("app.example.com").unwrap();
        assert!(render_route(&domain_id, &hostname, 0).is_err());
    }
}
