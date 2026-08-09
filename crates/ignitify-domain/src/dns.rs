use std::{fmt, net::Ipv4Addr};

use crate::{DomainName, InputError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DnsRecordType {
    A,
    Cname,
}

impl DnsRecordType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::A => "a",
            Self::Cname => "cname",
        }
    }
}

impl TryFrom<&str> for DnsRecordType {
    type Error = InputError;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "a" => Ok(Self::A),
            "cname" => Ok(Self::Cname),
            _ => Err(InputError::InvalidDnsRecordType),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DnsRecordTarget {
    Ipv4(Ipv4Addr),
    Hostname(DomainName),
}

impl fmt::Display for DnsRecordTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ipv4(address) => address.fmt(formatter),
            Self::Hostname(hostname) => hostname.fmt(formatter),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsRecord {
    record_type: DnsRecordType,
    target: DnsRecordTarget,
}

impl DnsRecord {
    pub fn new(record_type: DnsRecordType, target: impl AsRef<str>) -> Result<Self> {
        let target = target.as_ref().trim().to_ascii_lowercase();
        let target = match record_type {
            DnsRecordType::A => target
                .parse::<Ipv4Addr>()
                .map(DnsRecordTarget::Ipv4)
                .map_err(|_| InputError::InvalidDnsRecordTarget)?,
            DnsRecordType::Cname => DomainName::new(target)
                .map(DnsRecordTarget::Hostname)
                .map_err(|_| InputError::InvalidDnsRecordTarget)?,
        };
        Ok(Self {
            record_type,
            target,
        })
    }

    pub fn record_type(&self) -> DnsRecordType {
        self.record_type
    }

    pub fn target(&self) -> &DnsRecordTarget {
        &self.target
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DnsVerificationStatus {
    NotChecked,
    Pending,
    Valid,
    Missing,
    Unavailable,
}

impl DnsVerificationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotChecked => "not_checked",
            Self::Pending => "pending",
            Self::Valid => "valid",
            Self::Missing => "missing",
            Self::Unavailable => "unavailable",
        }
    }
}

impl TryFrom<&str> for DnsVerificationStatus {
    type Error = InputError;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "not_checked" => Ok(Self::NotChecked),
            "pending" => Ok(Self::Pending),
            "valid" => Ok(Self::Valid),
            "missing" => Ok(Self::Missing),
            "unavailable" => Ok(Self::Unavailable),
            _ => Err(InputError::InvalidDnsVerificationStatus),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DnsRecord, DnsRecordTarget, DnsRecordType};

    #[test]
    fn dns_records_validate_targets_by_record_type() {
        let record = DnsRecord::new(DnsRecordType::A, "203.0.113.10").unwrap();
        assert!(matches!(record.target(), DnsRecordTarget::Ipv4(_)));
        assert!(DnsRecord::new(DnsRecordType::A, "edge.example.com").is_err());
        assert!(DnsRecord::new(DnsRecordType::Cname, "edge.example.com").is_ok());
    }
}
