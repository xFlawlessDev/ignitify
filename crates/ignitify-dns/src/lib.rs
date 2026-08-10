use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use hickory_resolver::{
    Resolver, TokioResolver,
    config::{CLOUDFLARE, ResolverConfig},
    net::runtime::TokioRuntimeProvider,
    proto::rr::{RData, RecordType},
};
use ignitify_control_plane::{DnsVerificationResult, DnsVerifier};
use ignitify_domain::{DnsRecordTarget, DnsRecordType, DnsVerificationStatus};
use reqwest::{Client, header::ACCEPT};
use serde::Deserialize;

const CLOUDFLARE_DOH_ENDPOINT: &str = "https://cloudflare-dns.com/dns-query";
const CLOUDFLARE_DOH_HOST: &str = "cloudflare-dns.com";

fn cloudflare_doh_address() -> SocketAddr {
    SocketAddr::from(([1, 1, 1, 1], 443))
}

#[derive(Clone)]
pub struct SystemDnsVerifier {
    doh_client: Option<Client>,
    public_resolver: Option<TokioResolver>,
    system_resolver: Option<TokioResolver>,
}

impl SystemDnsVerifier {
    pub fn new() -> Self {
        let doh_client = Client::builder()
            .https_only(true)
            .resolve(CLOUDFLARE_DOH_HOST, cloudflare_doh_address())
            .timeout(Duration::from_secs(5))
            .build()
            .ok();
        let public_resolver = Resolver::builder_with_config(
            ResolverConfig::udp_and_tcp(&CLOUDFLARE),
            TokioRuntimeProvider::default(),
        )
        .build()
        .ok();
        let system_resolver = Resolver::builder_tokio()
            .ok()
            .and_then(|builder| builder.build().ok());
        Self {
            doh_client,
            public_resolver,
            system_resolver,
        }
    }
}

impl Default for SystemDnsVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl DnsVerifier for SystemDnsVerifier {
    async fn verify(&self, domain: &ignitify_db::DomainRecord) -> DnsVerificationResult {
        let Some(record) = &domain.dns_record else {
            return unavailable();
        };
        let hostname = format!("{}.", domain.hostname);
        match record.record_type() {
            DnsRecordType::A => {
                let DnsRecordTarget::Ipv4(expected) = record.target() else {
                    return unavailable();
                };
                if let Some(result) =
                    verify_a_doh_record(self.doh_client.as_ref(), &hostname, expected).await
                {
                    result
                } else if let Some(result) =
                    verify_a_record(self.public_resolver.as_ref(), &hostname, expected).await
                {
                    result
                } else {
                    verify_a_record(self.system_resolver.as_ref(), &hostname, expected)
                        .await
                        .unwrap_or_else(unavailable)
                }
            }
            DnsRecordType::Cname => {
                let DnsRecordTarget::Hostname(expected) = record.target() else {
                    return unavailable();
                };
                if let Some(result) =
                    verify_cname_doh_record(self.doh_client.as_ref(), &hostname, expected.as_str())
                        .await
                {
                    result
                } else if let Some(result) =
                    verify_cname_record(self.public_resolver.as_ref(), &hostname, expected.as_str())
                        .await
                {
                    result
                } else {
                    verify_cname_record(self.system_resolver.as_ref(), &hostname, expected.as_str())
                        .await
                        .unwrap_or_else(unavailable)
                }
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct DohResponse {
    status: u16,
    answer: Option<Vec<DohAnswer>>,
}

#[derive(Deserialize)]
struct DohAnswer {
    data: String,
}

async fn doh_lookup(
    client: Option<&Client>,
    hostname: &str,
    record_type: &str,
) -> Option<DohResponse> {
    let client = client?;
    let response = client
        .get(CLOUDFLARE_DOH_ENDPOINT)
        .query(&[("name", hostname), ("type", record_type)])
        .header(ACCEPT, "application/dns-json")
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.json().await.ok()
}

async fn verify_a_doh_record(
    client: Option<&Client>,
    hostname: &str,
    expected: &std::net::Ipv4Addr,
) -> Option<DnsVerificationResult> {
    let response = doh_lookup(client, hostname, "A").await?;
    verification_from_doh_response(response, |value| value == expected.to_string())
}

async fn verify_cname_doh_record(
    client: Option<&Client>,
    hostname: &str,
    expected: &str,
) -> Option<DnsVerificationResult> {
    let response = doh_lookup(client, hostname, "CNAME").await?;
    if expected.ends_with(".cfargotunnel.com") && is_flattened_cname_response(&response) {
        return verify_flattened_tunnel_cname(client, hostname).await;
    }
    verification_from_doh_response(response, |value| value.trim_end_matches('.') == expected)
}

async fn verify_flattened_tunnel_cname(
    client: Option<&Client>,
    hostname: &str,
) -> Option<DnsVerificationResult> {
    let response = doh_lookup(client, hostname, "A").await?;
    verification_from_flattened_tunnel_response(response)
}

fn is_flattened_cname_response(response: &DohResponse) -> bool {
    response.status == 0 && response.answer.as_ref().is_none_or(Vec::is_empty)
}

fn verification_from_doh_response(
    response: DohResponse,
    expected: impl Fn(&str) -> bool,
) -> Option<DnsVerificationResult> {
    match response.status {
        0 => Some(
            response
                .answer
                .as_deref()
                .is_some_and(|answers| answers.iter().any(|answer| expected(answer.data.trim())))
                .then(valid)
                .unwrap_or_else(missing),
        ),
        3 => Some(missing()),
        _ => None,
    }
}

fn verification_from_flattened_tunnel_response(
    response: DohResponse,
) -> Option<DnsVerificationResult> {
    match response.status {
        0 => Some(
            response
                .answer
                .as_deref()
                .is_some_and(|answers| {
                    answers
                        .iter()
                        .any(|answer| answer.data.trim().parse::<IpAddr>().is_ok())
                })
                .then(valid)
                .unwrap_or_else(missing),
        ),
        3 => Some(missing()),
        _ => None,
    }
}

async fn verify_a_record(
    resolver: Option<&TokioResolver>,
    hostname: &str,
    expected: &std::net::Ipv4Addr,
) -> Option<DnsVerificationResult> {
    let resolver = resolver?;
    match resolver.ipv4_lookup(hostname).await {
        Ok(lookup)
            if lookup.answers().iter().any(
                |answer| matches!(&answer.data, RData::A(address) if address.0 == *expected),
            ) =>
        {
            Some(valid())
        }
        Ok(_) => Some(missing()),
        Err(_) => None,
    }
}

async fn verify_cname_record(
    resolver: Option<&TokioResolver>,
    hostname: &str,
    expected: &str,
) -> Option<DnsVerificationResult> {
    let resolver = resolver?;
    match resolver.lookup(hostname, RecordType::CNAME).await {
        Ok(lookup)
            if lookup.answers().iter().any(|answer| {
                matches!(
                    &answer.data,
                    RData::CNAME(name)
                        if name.to_string().trim_end_matches('.') == expected
                )
            }) =>
        {
            Some(valid())
        }
        Ok(_) => Some(missing()),
        Err(_) => None,
    }
}

fn valid() -> DnsVerificationResult {
    DnsVerificationResult {
        status: DnsVerificationStatus::Valid,
        error: None,
    }
}

fn missing() -> DnsVerificationResult {
    DnsVerificationResult {
        status: DnsVerificationStatus::Missing,
        error: Some("the configured DNS record was not found".to_owned()),
    }
}

fn unavailable() -> DnsVerificationResult {
    DnsVerificationResult {
        status: DnsVerificationStatus::Unavailable,
        error: Some("the DNS resolver was unavailable".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::SystemDnsVerifier;

    #[test]
    fn configures_a_public_resolver_for_dns_verification() {
        let verifier = SystemDnsVerifier::new();

        assert!(verifier.doh_client.is_some());
        assert!(verifier.public_resolver.is_some());
    }

    #[test]
    fn validates_cname_records_from_a_dns_over_https_response() {
        let result = super::verification_from_doh_response(
            super::DohResponse {
                status: 0,
                answer: Some(vec![super::DohAnswer {
                    data: "tunnel.cfargotunnel.com.".to_owned(),
                }]),
            },
            |value| value.trim_end_matches('.') == "tunnel.cfargotunnel.com",
        );

        assert_eq!(
            result.expect("response should be definitive").status,
            ignitify_domain::DnsVerificationStatus::Valid
        );
    }

    #[test]
    fn accepts_a_flattened_cloudflare_tunnel_cname_when_the_hostname_resolves() {
        let result = super::verification_from_flattened_tunnel_response(super::DohResponse {
            status: 0,
            answer: Some(vec![super::DohAnswer {
                data: "104.16.0.1".to_owned(),
            }]),
        });

        assert_eq!(
            result.expect("response should be definitive").status,
            ignitify_domain::DnsVerificationStatus::Valid
        );
    }
}
