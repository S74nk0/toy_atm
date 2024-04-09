use super::common::{Amount, ClientID, TransactionID};
use serde::Deserialize;

/// [InputTransactionRecord](InputTransactionRecord) is used as a deserialization
/// helper struct ONLY and should not be used for anything else.
#[derive(Debug, serde::Deserialize)]
struct InputTransactionRecord {
    #[serde(rename = "type")]
    record_type: String,

    #[serde(rename = "client")]
    client_id: ClientID,

    #[serde(rename = "tx")]
    transaction_id: TransactionID,

    #[serde(rename = "amount")]
    amount: Option<Amount>,
}

/// [TransactionType] represants possible transaction types.
#[derive(Debug, Clone, Copy)]
pub enum TransactionType {
    /// [TransactionType::Deposit] represents the amount to credit an asset account.
    Deposit(Amount),

    /// Withdrawal represents the amount to debit an asset account.
    Withdrawal(Amount),

    /// Dispute represents a possible erronious transaction.
    Dispute,

    /// Resolve represents a resolution to a Dispute.
    Resolve,

    /// Chargeback represents a Dispute confirmation meaning that there was
    /// an erronious transaction.
    Chargeback,
}

/// [Transaction] represents a transaction type for a given
/// client ID and transactio ID. This will be usually be derived
/// from user/outside input (potentially untrused).
#[derive(Debug)]
pub struct Transaction {
    /// Represents the client ID.
    pub client_id: ClientID,

    /// Represents the transaction ID.
    pub transaction_id: TransactionID,

    /// Specifies the transaction type. The transaction type defines how to handle
    /// a given transaction.
    pub transaction_type: TransactionType,
}

impl<'de> Deserialize<'de> for Transaction {
    fn deserialize<D>(deserializer: D) -> Result<Transaction, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let tmp = InputTransactionRecord::deserialize(deserializer)?;
        use TransactionType::*;
        let client_id = tmp.client_id;
        let transaction_id = tmp.transaction_id;
        let transaction_type = match (tmp.record_type.as_str(), tmp.amount) {
            ("deposit", Some(amount)) => Deposit(amount),
            ("withdrawal", Some(amount)) => Withdrawal(amount),
            ("dispute", _) => Dispute,
            ("resolve", _) => Resolve,
            ("chargeback", _) => Chargeback,
            _ => {
                let missing_amount = tmp.amount.is_none();
                let err_msg = format!(
                    "Unknown type '{}' and/or missing amount '{}'",
                    &tmp.record_type, missing_amount
                );
                return Err(serde::de::Error::custom(err_msg));
            }
        };
        Ok(Transaction {
            client_id,
            transaction_id,
            transaction_type,
        })
    }
}
