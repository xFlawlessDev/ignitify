use chrono::Utc;
use ignitify_domain::{SupplyChainEnforcement, SupplyChainPolicy};
use sqlx::FromRow;

use crate::{DatabaseError, Result};

use super::DeploymentsRepository;

impl DeploymentsRepository {
    pub async fn supply_chain_policy(&self) -> Result<SupplyChainPolicy> {
        let row = sqlx::query_as::<_, SupplyChainPolicyRow>(
            "SELECT enforcement, updated_at FROM supply_chain_policy WHERE id = 1",
        )
        .fetch_one(&self.pool)
        .await?;
        row.into_policy()
    }

    pub async fn update_supply_chain_enforcement(
        &self,
        enforcement: SupplyChainEnforcement,
    ) -> Result<SupplyChainPolicy> {
        let updated_at = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE supply_chain_policy
             SET enforcement = ?, updated_at = ?
             WHERE id = 1",
        )
        .bind(enforcement.as_str())
        .bind(updated_at)
        .execute(&self.pool)
        .await?;
        self.supply_chain_policy().await
    }
}

#[derive(FromRow)]
struct SupplyChainPolicyRow {
    enforcement: String,
    updated_at: String,
}

impl SupplyChainPolicyRow {
    fn into_policy(self) -> Result<SupplyChainPolicy> {
        let enforcement = self
            .enforcement
            .as_str()
            .try_into()
            .map_err(|_| DatabaseError::InvalidSupplyChainEnforcement(self.enforcement))?;
        Ok(SupplyChainPolicy {
            enforcement,
            updated_at: self.updated_at,
        })
    }
}
