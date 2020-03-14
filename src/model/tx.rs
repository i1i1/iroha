use crate::model::{commands::isi::Command, crypto::Signature};
use std::{
    fmt::{Debug, Display, Formatter},
    time::SystemTime,
};

/// An ordered set of commands, which is applied to the ledger atomically.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Transaction {
    /// An ordered set of commands.
    //TODO: think about constructor with `Into<Command>` parameter signature.
    pub commands: Vec<Command>,
    /// Time of creation (unix time, in milliseconds).
    creation_time: u128,
    /// Account ID of transaction creator (username@domain).
    pub account_id: String,
    /// Quorum field (indicates required number of signatures).
    quorum: u32, //TODO: this will almost certainly change; accounts need conditional multisig based on some rules, not associated with a transaction
    pub signatures: Vec<Signature>,
}

impl Transaction {
    pub fn builder(commands: Vec<Command>, account_id: String) -> TxBuilder {
        TxBuilder {
            commands,
            account_id,
            creation_time: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("Failed to get System Time.")
                .as_millis(),
            ..Default::default()
        }
    }

    //TODO: make Transaction an Enum and return Transaction::Valid
    pub fn validate(self) -> Result<Transaction, ()> {
        Ok(self)
    }
}

/// Builder struct for `Transaction`.
#[derive(Default)]
pub struct TxBuilder {
    pub commands: Vec<Command>,
    pub creation_time: u128,
    pub account_id: String,
    pub quorum: Option<u32>,
    pub signatures: Option<Vec<Signature>>,
}

impl TxBuilder {
    pub fn build(self) -> Transaction {
        Transaction {
            commands: self.commands,
            creation_time: self.creation_time,
            account_id: self.account_id,
            quorum: self.quorum.unwrap_or(1),
            signatures: self.signatures.unwrap_or_default(),
        }
    }
}

impl Display for Transaction {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{:}", self.account_id) //TODO: implement
    }
}

impl Debug for Transaction {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{:}", self.account_id) //TODO: implement
    }
}
