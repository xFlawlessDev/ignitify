use hickory_resolver::{
    Resolver, TokioResolver,
    proto::rr::{RData, RecordType},
};
use ignitify_control_plane::{DnsVerificationResult, DnsVerifier};
use ignitify_domain::{DnsRecordTarget, DnsRecordType, DnsVerificationStatus};

#[derive(Clone)]
pub struct SystemDnsVerifier {
    resolver: Option<TokioResolver>,
}

impl SystemDnsVerifier {
    pub fn new() -> Self {
        let resolver = Resolver::builder_tokio()
            .ok()
            .and_then(|builder| builder.build().ok());
        Self { resolver }
    }
}

impl Default for SystemDnsVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl DnsVerifier for SystemDnsVerifier {
    async fn verify(&self, domain: &ignitify_db::DomainRecord) -> DnsVerificationResult {
        let Some(resolver) = &self.resolver else {
            return unavailable();
        };
        let Some(record) = &domain.dns_record else {
            return unavailable();
        };
        let hostname = format!("{}.", domain.hostname);
        match record.record_type() {
            DnsRecordType::A => {
                let DnsRecordTarget::Ipv4(expected) = record.target() else {
                    return unavailable();
                };
                match resolver.ipv4_lookup(hostname).await {
                    Ok(lookup)
                        if lookup.answers().iter().any(|answer| {
                            matches!(&answer.data, RData::A(address) if address.0 == *expected)
                        }) =>
                    {
                        valid()
                    }
                    Ok(_) => missing(),
                    Err(_) => unavailable(),
                }
            }
            DnsRecordType::Cname => {
                let DnsRecordTarget::Hostname(expected) = record.target() else {
                    return unavailable();
                };
                match resolver.lookup(hostname, RecordType::CNAME).await {
                    Ok(lookup)
                        if lookup.answers().iter().any(|answer| {
                            matches!(
                                &answer.data,
                                RData::CNAME(name)
                                    if name.to_string().trim_end_matches('.') == expected.as_str()
                            )
                        }) =>
                    {
                        valid()
                    }
                    Ok(_) => missing(),
                    Err(_) => unavailable(),
                }
            }
        }
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
