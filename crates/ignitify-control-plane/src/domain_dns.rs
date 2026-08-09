use std::future::Future;

use ignitify_db::{DomainRecord, DomainsRepository, Result as DatabaseResult};
use ignitify_domain::DnsVerificationStatus;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsVerificationResult {
    pub status: DnsVerificationStatus,
    pub error: Option<String>,
}

pub trait DnsVerifier: Send + Sync + 'static {
    fn verify(&self, domain: &DomainRecord) -> impl Future<Output = DnsVerificationResult> + Send;
}

#[derive(Clone, Copy, Default)]
pub struct NoopDnsVerifier;

impl DnsVerifier for NoopDnsVerifier {
    async fn verify(&self, _domain: &DomainRecord) -> DnsVerificationResult {
        DnsVerificationResult {
            status: DnsVerificationStatus::Unavailable,
            error: Some("DNS verification is not configured".to_owned()),
        }
    }
}

pub async fn reconcile_dns_verifications<V>(
    domains: &DomainsRepository,
    verifier: &V,
) -> DatabaseResult<()>
where
    V: DnsVerifier,
{
    for domain in domains.pending_dns_verifications().await? {
        let result = verifier.verify(&domain).await;
        domains
            .complete_dns_verification(domain.id.as_str(), result.status, result.error.as_deref())
            .await?;
    }
    Ok(())
}
