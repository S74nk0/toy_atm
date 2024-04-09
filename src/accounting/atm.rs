use std::collections::HashMap;

use serde::Serialize;

use crate::accounting::transaction::TransactionType;

use super::{
    common::{Amount, ClientID, TransactionID},
    transaction::Transaction,
};

/// [IgnoredTransactionReason] states the reason why a transaction was ignored.
/// [IgnoredTransactionReason] represents an error where we can assume that the
/// account balance was not modified.
#[derive(Debug, PartialEq)]
pub enum IgnoredTransactionReason {
    /// LockedAccount represents that the account balance is locked/frozen and
    /// that this is the reason for ignoring the transaction.
    LockedAccount,

    /// NegativeAmount represents that the provided credit or debit amount is a
    /// negative number. Since we rely on the sign representing credits or debits
    /// we don't want to accept negative values.
    NegativeAmount,

    /// ZeroAmount represents that the provided credit or debit amount is 0.
    /// This could be acceptet but since this does not actually change the state
    /// of the account balance we choose to interpert this as an error or ignored
    /// transaction.
    ZeroAmount,

    /// DuplicateTransactionIDInsertion represents that the
    /// [TransactionID] already exists and has been rejected.
    /// This can occur for Deposits and Withdrawals.
    DuplicateTransactionIDInsertion,

    /// InsufficientAvailableFunds represents that there was a Withdrawal with
    /// a larger amount than the available balance.
    InsufficientAvailableFunds,

    /// MissingTransactionID represents a missing [TransactionID]
    /// for a Dispute, Resolve or Chargeback and that there is nothing to
    /// transition to.
    MissingTransactionID,

    /// NoTransactionStateChange represents that the transaction transition
    /// state is unchanged. This is not an error it is just to state why it was
    /// ignored and that the account balance is unchanged.
    NoTransactionStateChange,

    /// InvalidTransactionStateTransition represents that the transaction could
    /// not be transitioned from the current state to the new state. This is
    /// triggered by one of the following Dispute, Resolve or Chargeback.
    InvalidTransactionStateTransition,
}

/// [InvalidClientBalance] indicates that the account balance is in an invalid state.
#[derive(Debug, PartialEq)]
pub enum InvalidClientBalance {
    InvalidAvailableAmount,
    InvalidHeldAmount,
    InvalidTotalAmount,
}

/// [HandledTransactionError] represents a handled transaction that was erroneous
/// and states what went wrong.
/// We can have two types of errors:
///   - [HandledTransactionError::IgnoredTransactionReason]
///   - [HandledTransactionError::InvalidClientBalance]
#[derive(Debug, PartialEq)]
pub enum HandledTransactionError {
    /// [HandledTransactionError::IgnoredTransactionReason] indicates that the transaction was ignored and
    /// did not change any account balance.
    IgnoredTransactionReason(TransactionID, IgnoredTransactionReason),

    /// [HandledTransactionError::InvalidClientBalance] indicates that the transaction was handled and it
    /// caused an invalid account balance change.
    InvalidClientBalance(TransactionID, InvalidClientBalance),
}

impl From<(TransactionID, IgnoredTransactionReason)> for HandledTransactionError {
    fn from(pair: (TransactionID, IgnoredTransactionReason)) -> Self {
        let (transaction_id, ignored_reason) = pair;
        Self::IgnoredTransactionReason(transaction_id, ignored_reason)
    }
}

pub type HandledTransactionResult = Result<(), HandledTransactionError>;

/// [TransactionState] is used to represent a debit or credit state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransactionState {
    /// [TransactionState::Disputed] indicates there was a Dispute
    Disputed,

    /// [TransactionState::Resolved] indicates there was a Resolve to a Dispute
    /// or we assume all initial debits and credits are in a resolved state.
    Resolved,

    /// [TransactionState::Chargeback] indicates the final state of a Dispute.
    Chargeback,
}

enum TransactionStateTransition {
    NoOperation,
    Invalid,
    Valid,
}

impl TransactionState {
    fn calc_transition(from: &Self, to: &Self) -> TransactionStateTransition {
        use TransactionState::*;
        use TransactionStateTransition::*;
        match (from, to) {
            (Disputed, Disputed) => NoOperation,
            (Disputed, Resolved) => Valid,
            (Disputed, Chargeback) => Valid,
            (Resolved, Disputed) => Valid,
            (Resolved, Resolved) => NoOperation,
            (Resolved, Chargeback) => Invalid,
            (Chargeback, _) => Invalid,
        }
    }
}

/// [CreditDebitState] holds debit and credit amounts with transaction state.
#[derive(Debug)]
enum CreditDebitState {
    Deposit(Amount, TransactionState),
    Withdrawal(Amount, TransactionState),
}

impl CreditDebitState {
    fn deposit(amount: Amount) -> Self {
        Self::Deposit(amount, TransactionState::Resolved)
    }

    fn withdrawal(amount: Amount) -> Self {
        Self::Withdrawal(amount, TransactionState::Resolved)
    }

    fn get_credit_or_debit_reverse_amount(&self) -> Amount {
        match &self {
            Self::Deposit(amount, _) => *amount,
            Self::Withdrawal(amount, _) => amount.reversed(),
        }
    }

    fn get_transaction_state(&self) -> TransactionState {
        match &self {
            Self::Deposit(_, state) => *state,
            Self::Withdrawal(_, state) => *state,
        }
    }

    fn set_transaction_state(&mut self, to: TransactionState) {
        match self {
            Self::Deposit(_, state) => *state = to,
            Self::Withdrawal(_, state) => *state = to,
        }
    }
}

#[derive(Debug, PartialEq, Serialize)]
pub struct ClientBalanceSnapshot {
    #[serde(rename = "client")]
    client_id: ClientID,

    #[serde(rename = "available")]
    available: Amount,

    #[serde(rename = "held")]
    held: Amount,

    #[serde(rename = "total")]
    total: Amount,

    #[serde(rename = "locked")]
    locked: bool,
}

// #[derive(Debug, Default)]
// struct CreditDebitBalance {
//     available: Amount,

//     held: Amount,

//     total: Amount,
// }

#[derive(Debug, Default)]
pub struct ClientBalance {
    client_id: ClientID,

    available: Amount,

    held: Amount,

    total: Amount,

    locked: bool,

    // credit_balance: CreditDebitBalance,

    // debit_balance: CreditDebitBalance,
    transactions: HashMap<TransactionID, CreditDebitState>,
}

impl ClientBalance {
    pub fn client_balance_snapshot(&self) -> ClientBalanceSnapshot {
        ClientBalanceSnapshot {
            client_id: self.client_id,
            available: self.available,
            held: self.held,
            total: self.total,
            locked: self.locked,
        }
    }
    fn is_valid(&self) -> Result<(), InvalidClientBalance> {
        use InvalidClientBalance::*;
        let available = self.total - self.held;
        if !self.available.eq(&available) {
            return Err(InvalidAvailableAmount);
        }
        let held = self.total - self.available;
        if !self.held.eq(&held) {
            return Err(InvalidHeldAmount);
        }
        let total = self.available + self.held;
        if !self.total.eq(&total) {
            return Err(InvalidTotalAmount);
        }
        Ok(())
    }

    pub fn handle_transaction(&mut self, tx: Transaction) -> HandledTransactionResult {
        let transaction_id = tx.transaction_id;
        let transaction_type = tx.transaction_type;
        if self.locked {
            return Err((transaction_id, IgnoredTransactionReason::LockedAccount).into());
        }

        use TransactionType::*;
        let handled_tx_result = match transaction_type {
            Deposit(credit_amount) => self.handle_deposit(transaction_id, credit_amount),
            Withdrawal(debit_amount) => self.handle_withdrawal(transaction_id, debit_amount),
            Dispute => self.handle_dispute(transaction_id),
            Resolve => self.handle_resolve(transaction_id),
            Chargeback => self.handle_chargeback(transaction_id),
        };
        if let Err(ignore_err) = handled_tx_result {
            return Err((transaction_id, ignore_err).into());
        }
        if let Err(err) = self.is_valid() {
            return Err(HandledTransactionError::InvalidClientBalance(
                transaction_id,
                err,
            ));
        }

        Ok(())
    }

    fn handle_deposit(
        &mut self,
        transaction_id: TransactionID,
        amount: Amount,
    ) -> Result<(), IgnoredTransactionReason> {
        self.handle_deposit_or_withdrawal_insertion(transaction_id, amount, false)
    }

    fn handle_withdrawal(
        &mut self,
        transaction_id: TransactionID,
        amount: Amount,
    ) -> Result<(), IgnoredTransactionReason> {
        self.handle_deposit_or_withdrawal_insertion(transaction_id, amount, true)
    }

    fn handle_deposit_or_withdrawal_insertion(
        &mut self,
        transaction_id: TransactionID,
        amount: Amount,
        is_withdrawal: bool,
    ) -> Result<(), IgnoredTransactionReason> {
        use IgnoredTransactionReason::*;
        if amount.is_negative() {
            return Err(NegativeAmount);
        }
        if amount.is_zero() {
            return Err(ZeroAmount);
        }
        if self.transactions.contains_key(&transaction_id) {
            return Err(DuplicateTransactionIDInsertion);
        }
        if is_withdrawal && self.available < amount  {
            return Err(InsufficientAvailableFunds);
        }

        // execute deposit or withdrawal
        if is_withdrawal {
            self.transactions
                .insert(transaction_id, CreditDebitState::withdrawal(amount));

            self.available -= amount;
            self.total -= amount;

            // // debit balance
            // self.debit_balance.available += amount;
            // self.debit_balance.total += amount;
        } else {
            self.transactions
                .insert(transaction_id, CreditDebitState::deposit(amount));

            self.available += amount;
            self.total += amount;

            // // credit balance
            // self.credit_balance.available += amount;
            // self.credit_balance.total += amount;
        }

        Ok(())
    }

    fn handle_dispute(
        &mut self,
        transaction_id: TransactionID,
    ) -> Result<(), IgnoredTransactionReason> {
        self.handle_transaction_trasition(transaction_id, TransactionState::Disputed)
    }

    fn handle_resolve(
        &mut self,
        transaction_id: TransactionID,
    ) -> Result<(), IgnoredTransactionReason> {
        self.handle_transaction_trasition(transaction_id, TransactionState::Resolved)
    }

    fn handle_chargeback(
        &mut self,
        transaction_id: TransactionID,
    ) -> Result<(), IgnoredTransactionReason> {
        self.handle_transaction_trasition(transaction_id, TransactionState::Chargeback)
    }

    fn handle_transaction_trasition(
        &mut self,
        transaction_id: TransactionID,
        to: TransactionState,
    ) -> Result<(), IgnoredTransactionReason> {
        use IgnoredTransactionReason::*;
        let Some(tx) = self.transactions.get_mut(&transaction_id) else {
            return Err(MissingTransactionID);
        };
        let from = tx.get_transaction_state();
        use TransactionStateTransition::*;
        match TransactionState::calc_transition(&from, &to) {
            NoOperation => return Err(NoTransactionStateChange),
            Invalid => return Err(InvalidTransactionStateTransition),
            Valid => {
                tx.set_transaction_state(to);
            }
        }
        // execute balance change
        let amount = tx.get_credit_or_debit_reverse_amount();
        use TransactionState::*;
        match to {
            Disputed => {
                self.available -= amount;
                self.held += amount;
            }
            Resolved => {
                self.available += amount;
                self.held -= amount;
            }
            Chargeback => {
                self.locked = true;
                self.total -= amount;
                self.held -= amount;
            }
        }

        // match *tx {
        //     CreditDebitState::Deposit(amount, _) => {
        //         match to {
        //             Disputed => {
        //                 self.credit_balance.available -= amount;
        //                 self.credit_balance.held += amount;
        //             }
        //             Resolved => {
        //                 self.credit_balance.available += amount;
        //                 self.credit_balance.held -= amount;
        //             }
        //             Chargeback => {
        //                 self.locked = true;
        //                 self.credit_balance.total -= amount;
        //                 self.credit_balance.held -= amount;
        //             }
        //         }
        //     },
        //     CreditDebitState::Withdrawal(amount, _) => {
        //         match to {
        //             Disputed => {
        //                 self.debit_balance.available -= amount;
        //                 self.debit_balance.held += amount;
        //             }
        //             Resolved => {
        //                 self.debit_balance.available += amount;
        //                 self.debit_balance.held -= amount;
        //             }
        //             Chargeback => {
        //                 self.locked = true;
        //                 self.debit_balance.total -= amount;
        //                 self.debit_balance.held -= amount;
        //             }
        //         }
        //     },
        // }

        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct Atm {
    client_balances: HashMap<ClientID, ClientBalance>,
}

impl Atm {
    pub fn handle_transaction(&mut self, tx: Transaction) -> HandledTransactionResult {
        // get or create records for client
        let client_balance = self
            .client_balances
            .entry(tx.client_id)
            .or_insert(ClientBalance {
                client_id: tx.client_id,
                ..Default::default()
            });
        client_balance.handle_transaction(tx)
    }

    pub fn accounts(&self) -> impl Iterator<Item = ClientBalanceSnapshot> + '_ {
        self.client_balances
            .values()
            .map(|cb| cb.client_balance_snapshot())
    }
}

// tests

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use crate::accounting::{
        atm::{Atm, CreditDebitState, HandledTransactionError, IgnoredTransactionReason, TransactionState},
        common::{Amount, ClientID, TransactionID},
        transaction::{self, Transaction, TransactionType},
    };

    use super::ClientBalance;
    use proptest::prelude::*;

    #[derive(Debug, PartialEq)]
    struct ClientBalanceSnapshot(Amount, Amount, Amount, bool);

    /// [ClientBalanceTestWrapper] is a wrapper for testing [ClientBalance]
    /// transaction handling.
    struct ClientBalanceTestWrapper {
        cb: ClientBalance,
        last_saved_client_balance_snapshot: ClientBalanceSnapshot,
    }

    impl ClientBalanceTestWrapper {
        fn new() -> Self {
            let cb = ClientBalance::default();
            let last_saved_client_balance_snapshot =
                ClientBalanceSnapshot(cb.available, cb.held, cb.total, cb.locked);
            Self {
                cb,
                last_saved_client_balance_snapshot,
            }
        }

        fn current_client_balance_snapshot(&self) -> ClientBalanceSnapshot {
            ClientBalanceSnapshot(
                self.cb.available,
                self.cb.held,
                self.cb.total,
                self.cb.locked,
            )
        }

        fn assert_frozen_account(&self) {
            assert_eq!(
                self.cb.locked, true,
                "assert_frozen_account expecting locked to be true"
            );
        }
        fn assert_unlocked_account(&self) {
            assert_eq!(
                self.cb.locked, false,
                "assert_unlocked_account expecting locked to be false"
            );
        }

        fn assert_ok_transaction(
            &mut self,
            transaction_id: TransactionID,
            transaction_type: TransactionType,
        ) {
            let tx = Transaction {
                client_id: self.cb.client_id,
                transaction_id,
                transaction_type,
            };
            let res = self.cb.handle_transaction(tx);
            assert_eq!(res, Ok(()), "assert_ok_transaction expecting ok");
            let mut new = self.current_client_balance_snapshot();
            assert_ne!(
                new, self.last_saved_client_balance_snapshot,
                "assert_ok_transaction client balance snapshots expected to differ (for non zero amounts)"
            );
            std::mem::swap(&mut new, &mut self.last_saved_client_balance_snapshot);
        }

        fn assert_ok_transaction_and_assert_frozen_account(
            &mut self,
            transaction_id: TransactionID,
            transaction_type: TransactionType,
        ) {
            self.assert_ok_transaction(transaction_id, transaction_type);
            self.assert_frozen_account()
        }

        fn assert_ok_transaction_and_assert_unlocked_account(
            &mut self,
            transaction_id: TransactionID,
            transaction_type: TransactionType,
        ) {
            self.assert_ok_transaction(transaction_id, transaction_type);
            self.assert_unlocked_account()
        }

        fn assert_err_transaction_ignored(
            &mut self,
            transaction_id: TransactionID,
            transaction_type: TransactionType,
        ) -> IgnoredTransactionReason {
            let tx = Transaction {
                client_id: self.cb.client_id,
                transaction_id,
                transaction_type,
            };
            let res = self.cb.handle_transaction(tx);
            let new = self.current_client_balance_snapshot();
            assert_eq!(
                new, self.last_saved_client_balance_snapshot,
                "assert_err_transaction_ignored client balance snapshots expected be equal for ignored transactions"
            );
            match res {
                Ok(_) => panic!(
                    "assert_err_transaction_ignored expecting error but the result returned OK"
                ),
                Err(err) => match err {
                    HandledTransactionError::IgnoredTransactionReason(
                        transaction_id2,
                        ignore_err,
                    ) => {
                        assert_eq!(
                            transaction_id, transaction_id2,
                            "assert_err_transaction_ignored tx id missmatch"
                        );
                        ignore_err
                    }
                    HandledTransactionError::InvalidClientBalance(_, _) => {
                        // if we get this then the whole thing is basically not working
                        panic!("assert_err_transaction_ignored got invalid client balance!!!")
                    }
                },
            }
        }
    }

    #[test]
    fn test_zero_client_balance() {
        let cb = ClientBalance::default();
        let res = cb.is_valid();
        assert!(res.is_ok(), "default client balance should return ok")
    }

    #[test]
    fn test_zero_amount_deposit_withdrawal() {
        use IgnoredTransactionReason::*;
        use TransactionType::*;
        let mut cb_test_w = ClientBalanceTestWrapper::new();
        let mut transaction_id = TransactionID::default();
        let amount = Amount::new(0.0);
        let transactions = vec![Deposit(amount), Withdrawal(amount)];

        for tx_type in transactions {
            transaction_id.increase_by_one();
            let ignored = cb_test_w.assert_err_transaction_ignored(transaction_id, tx_type);
            assert_eq!(ignored, ZeroAmount, "expecting error for zero amount");
        }
    }

    #[test]
    fn test_negative_amount_deposit_withdrawal() {
        use IgnoredTransactionReason::*;
        use TransactionType::*;
        let mut cb_test_w = ClientBalanceTestWrapper::new();
        let mut transaction_id = TransactionID::default();
        let amount = Amount::new(-1.0);
        let transactions = vec![Deposit(amount), Withdrawal(amount)];

        for tx_type in transactions {
            transaction_id.increase_by_one();
            let ignored = cb_test_w.assert_err_transaction_ignored(transaction_id, tx_type);
            assert_eq!(
                ignored, NegativeAmount,
                "expecting error for negative amount"
            );
        }
    }

    #[test]
    fn test_positive_amount_deposit() {
        use TransactionType::*;
        let mut cb_test_w = ClientBalanceTestWrapper::new();
        let transaction_id = TransactionID::default();
        let deposit = Deposit(Amount::new(1.0));
        cb_test_w.assert_ok_transaction_and_assert_unlocked_account(transaction_id, deposit);
    }

    #[test]
    fn test_duplicate_transaction_id_deposit() {
        use IgnoredTransactionReason::*;
        use TransactionType::*;
        let mut cb_test_w = ClientBalanceTestWrapper::new();
        let transaction_id = TransactionID::default();
        let deposit_amout = Amount::new(100.0);

        cb_test_w.assert_ok_transaction_and_assert_unlocked_account(
            transaction_id,
            Deposit(deposit_amout),
        );

        let res = cb_test_w.assert_err_transaction_ignored(transaction_id, Deposit(deposit_amout));
        assert_eq!(
            res, DuplicateTransactionIDInsertion,
            "expecting error for duplicated transaction_id"
        );
    }

    #[test]
    fn test_duplicate_transaction_id_withdrawal() {
        use IgnoredTransactionReason::*;
        use TransactionType::*;
        let mut cb_test_w = ClientBalanceTestWrapper::new();
        let transaction_id = TransactionID::default();
        let deposit_amout = Amount::new(100.0);

        cb_test_w.assert_ok_transaction_and_assert_unlocked_account(
            transaction_id,
            Deposit(deposit_amout),
        );

        let transaction_id = transaction_id.next();
        cb_test_w.assert_ok_transaction_and_assert_unlocked_account(
            transaction_id,
            Withdrawal(deposit_amout),
        );
        let res =
            cb_test_w.assert_err_transaction_ignored(transaction_id, Withdrawal(deposit_amout));
        assert_eq!(
            res, DuplicateTransactionIDInsertion,
            "expecting error for duplicated transaction_id"
        );
    }

    #[test]
    fn test_insufficient_amount_withdrawal() {
        use IgnoredTransactionReason::*;
        use TransactionType::*;
        let mut cb_test_w = ClientBalanceTestWrapper::new();
        let mut transaction_id = TransactionID::default();
        let amount = Amount::new(100.0);

        // withdrawal when empty
        let res = cb_test_w.assert_err_transaction_ignored(transaction_id, Withdrawal(amount));
        assert_eq!(
            res, InsufficientAvailableFunds,
            "expecting error for insufficient funds"
        );

        // deposit
        transaction_id.increase_by_one();
        cb_test_w
            .assert_ok_transaction_and_assert_unlocked_account(transaction_id, Deposit(amount));

        // withdrawal over
        let amount = amount + Amount::new(10.0);
        transaction_id.increase_by_one();
        let res = cb_test_w.assert_err_transaction_ignored(transaction_id, Withdrawal(amount));
        assert_eq!(
            res, InsufficientAvailableFunds,
            "expecting error for insufficient"
        );
    }

    #[test]
    fn test_empty_dispute_resolve_chargeback() {
        use IgnoredTransactionReason::*;
        use TransactionType::*;
        let mut cb_test_w = ClientBalanceTestWrapper::new();
        let transaction_id = TransactionID::default();
        let transactions = vec![Dispute, Resolve, Chargeback];

        for tx_type in transactions {
            let ignored = cb_test_w.assert_err_transaction_ignored(transaction_id, tx_type);
            assert_eq!(
                ignored, MissingTransactionID,
                "expecting error for insufficient"
            );
        }
    }

    #[test]
    fn test_no_transaction_change_dispute_resolve_chargeback() {
        use IgnoredTransactionReason::*;
        use TransactionType::*;
        let mut cb_test_w = ClientBalanceTestWrapper::new();
        let transaction_id = TransactionID::default();
        let transactions = vec![Dispute, Resolve];
        cb_test_w.assert_ok_transaction_and_assert_unlocked_account(
            transaction_id,
            Deposit(Amount::new(100.0)),
        );
        for tx_type in transactions {
            cb_test_w.assert_ok_transaction_and_assert_unlocked_account(transaction_id, tx_type);

            let ignored = cb_test_w.assert_err_transaction_ignored(transaction_id, tx_type);
            assert_eq!(ignored, NoTransactionStateChange);
        }
    }

    #[test]
    fn test_invalid_resolve_chargeback_transitions() {
        use IgnoredTransactionReason::*;
        use TransactionType::*;
        let mut cb_test_w = ClientBalanceTestWrapper::new();
        let mut transaction_id = TransactionID::default();
        let amount = Amount::new(100.0);

        let insert_transcations = vec![Deposit(amount), Withdrawal(amount)];
        let transition_transactions = vec![Resolve, Chargeback];
        let transition_ignore_results =
            vec![NoTransactionStateChange, InvalidTransactionStateTransition];
        for insert in insert_transcations {
            transaction_id.increase_by_one();
            let transaction_id = transaction_id;
            cb_test_w.assert_ok_transaction_and_assert_unlocked_account(transaction_id, insert);
            for transition in transition_transactions
                .iter()
                .zip(&transition_ignore_results)
            {
                let (transition, ignore_err) = transition;
                let ignored = cb_test_w.assert_err_transaction_ignored(transaction_id, *transition);
                assert_eq!(ignored, *ignore_err);
            }
        }
    }

    #[test]
    fn test_dispute_deposit() {
        use TransactionType::*;
        let mut cb_test_w = ClientBalanceTestWrapper::new();
        let transaction_id = TransactionID::default();
        let deposit_amout = Amount::new(100.0);

        cb_test_w.assert_ok_transaction_and_assert_unlocked_account(
            transaction_id,
            Deposit(deposit_amout),
        );
        cb_test_w.assert_ok_transaction_and_assert_unlocked_account(transaction_id, Dispute);
    }

    #[test]
    fn test_dispute_resolve_deposit() {
        use TransactionType::*;
        let mut cb_test_w = ClientBalanceTestWrapper::new();
        let transaction_id = TransactionID::default();
        let deposit_amout = Amount::new(100.0);

        cb_test_w.assert_ok_transaction_and_assert_unlocked_account(
            transaction_id,
            Deposit(deposit_amout),
        );
        cb_test_w.assert_ok_transaction_and_assert_unlocked_account(transaction_id, Dispute);
        cb_test_w.assert_ok_transaction_and_assert_unlocked_account(transaction_id, Resolve);
    }

    #[test]
    fn test_dispute_resolve_deposit_with_withdrawal() {
        use IgnoredTransactionReason::*;
        use TransactionType::*;
        let mut cb_test_w = ClientBalanceTestWrapper::new();
        let transaction_id = TransactionID::default();
        let deposit_amout = Amount::new(100.0);

        cb_test_w.assert_ok_transaction_and_assert_unlocked_account(
            transaction_id,
            Deposit(deposit_amout),
        );
        cb_test_w.assert_ok_transaction_and_assert_unlocked_account(transaction_id, Dispute);
        let ignored = cb_test_w.assert_err_transaction_ignored(transaction_id.next(), Withdrawal(deposit_amout));
        assert_eq!(ignored, InsufficientAvailableFunds);
        cb_test_w.assert_ok_transaction_and_assert_unlocked_account(transaction_id, Resolve);
        cb_test_w.assert_ok_transaction_and_assert_unlocked_account(transaction_id.next(), Withdrawal(deposit_amout));
    }

    #[test]
    fn test_dispute_chargeback_deposit() {
        use IgnoredTransactionReason::*;
        use TransactionType::*;
        let mut cb_test_w = ClientBalanceTestWrapper::new();
        let transaction_id = TransactionID::default();
        let amount = Amount::new(100.0);

        cb_test_w
            .assert_ok_transaction_and_assert_unlocked_account(transaction_id, Deposit(amount));
        cb_test_w.assert_ok_transaction_and_assert_unlocked_account(transaction_id, Dispute);
        cb_test_w.assert_ok_transaction_and_assert_frozen_account(transaction_id, Chargeback);

        let transactions = vec![Withdrawal(amount), Deposit(amount), Dispute, Resolve, Chargeback];
        for transaction_type in transactions {
            let ignored = cb_test_w.assert_err_transaction_ignored(transaction_id.next(), transaction_type);
            assert_eq!(ignored, LockedAccount, "expecting error locked account");
        }
    }

    #[test]
    fn test_deposit_withdrawal_dispute() {
        use TransactionType::*;
        let mut cb_test_w = ClientBalanceTestWrapper::new();
        let transaction_id = TransactionID::default();
        let deposit_amout = Amount::new(100.0);

        cb_test_w.assert_ok_transaction_and_assert_unlocked_account(
            transaction_id,
            Deposit(deposit_amout),
        );

        let transaction_id = transaction_id.next();
        let withdrawal_amout = Amount::new(100.0);
        cb_test_w.assert_ok_transaction_and_assert_unlocked_account(
            transaction_id,
            Withdrawal(withdrawal_amout),
        );
        cb_test_w.assert_ok_transaction_and_assert_unlocked_account(transaction_id, Dispute);
    }

    #[test]
    fn test_deposit_withdrawal_dispute_resolve() {
        use TransactionType::*;
        let mut cb_test_w = ClientBalanceTestWrapper::new();
        let transaction_id = TransactionID::default();
        let deposit_amout = Amount::new(100.0);

        cb_test_w.assert_ok_transaction_and_assert_unlocked_account(
            transaction_id,
            Deposit(deposit_amout),
        );

        let transaction_id = transaction_id.next();
        let withdrawal_amout = Amount::new(100.0);
        cb_test_w.assert_ok_transaction_and_assert_unlocked_account(
            transaction_id,
            Withdrawal(withdrawal_amout),
        );
        cb_test_w.assert_ok_transaction_and_assert_unlocked_account(transaction_id, Dispute);
        cb_test_w.assert_ok_transaction_and_assert_unlocked_account(transaction_id, Resolve);
    }

    #[test]
    fn test_deposit_withdrawal_dispute_chargeback() {
        use IgnoredTransactionReason::*;
        use TransactionType::*;
        let mut cb_test_w = ClientBalanceTestWrapper::new();
        let transaction_id = TransactionID::default();
        let deposit_amout = Amount::new(100.0);

        cb_test_w.assert_ok_transaction_and_assert_unlocked_account(
            transaction_id,
            Deposit(deposit_amout),
        );

        let transaction_id = transaction_id.next();
        let withdrawal_amout = Amount::new(100.0);
        cb_test_w.assert_ok_transaction_and_assert_unlocked_account(
            transaction_id,
            Withdrawal(withdrawal_amout),
        );
        cb_test_w.assert_ok_transaction_and_assert_unlocked_account(transaction_id, Dispute);
        cb_test_w.assert_ok_transaction_and_assert_frozen_account(transaction_id, Chargeback);
        
        let transactions = vec![Withdrawal(10.0.into()), Deposit(10.0.into()), Dispute, Resolve, Chargeback];
        for transaction_type in transactions {
            let ignored = cb_test_w.assert_err_transaction_ignored(transaction_id.next(), transaction_type);
            assert_eq!(ignored, LockedAccount, "expecting error locked account");
        }
    }

    // more tests with generated inputs
    #[test]
    fn test_deposits_only() {
        let cb_test_w = RefCell::new(ClientBalanceTestWrapper::new());
        let global_tx_id_seq = RefCell::new(TransactionID::default());
        proptest!(|(amount in 1f64..1000.0)| {
            let transaction_id = {
                let mut tx_id_seq = global_tx_id_seq.borrow_mut();
                tx_id_seq.increase_by_one();
                *tx_id_seq
            };
            let deposit = TransactionType::Deposit(amount.into());

            let mut cb_test_w = cb_test_w.borrow_mut();
            cb_test_w.assert_ok_transaction_and_assert_unlocked_account(transaction_id, deposit);
        });
    }

    #[test]
    fn test_deposits_and_withdrawals_equal_amounts_only() {
        use IgnoredTransactionReason::*;
        use TransactionType::*;
        let cb_test_w = RefCell::new(ClientBalanceTestWrapper::new());
        let global_tx_id_seq = RefCell::new(TransactionID::default());

        proptest!(|(amount in 1f64..1000.0)| {            
            let deposit_transaction_id = {
                let mut tx_id_seq = global_tx_id_seq.borrow_mut();
                tx_id_seq.increase_by_one();
                *tx_id_seq
            };
            let amount = amount.into();
            let deposit_amout = Deposit(amount);
            
            let withdrawal_transaction_id = {
                let mut tx_id_seq = global_tx_id_seq.borrow_mut();
                tx_id_seq.increase_by_one();
                *tx_id_seq
            };
            let withdrawal_amout = Withdrawal(amount);
            
            let mut cb = cb_test_w.borrow_mut();
            cb.assert_ok_transaction_and_assert_unlocked_account(deposit_transaction_id, deposit_amout);
            cb.assert_ok_transaction_and_assert_unlocked_account(withdrawal_transaction_id, withdrawal_amout);
        });
    }

    #[test]
    fn test_deposits_and_withdrawals_lower_amounts_only() {
        use TransactionType::*;
        let cb_test_w = RefCell::new(ClientBalanceTestWrapper::new());
        let global_tx_id_seq = RefCell::new(TransactionID::default());
        let amount_count = RefCell::new(0);

        proptest!(|(amount in 1f64..1000.0)| {
            let mut amount_count = amount_count.borrow_mut();
            *amount_count += 1u64;
            let deposit_transaction_id = {
                let mut tx_id_seq = global_tx_id_seq.borrow_mut();
                tx_id_seq.increase_by_one();
                *tx_id_seq
            };
            let deposit_amout = Deposit(Amount::new(amount + 1.0));
            
            let withdrawal_transaction_id = {
                let mut tx_id_seq = global_tx_id_seq.borrow_mut();
                tx_id_seq.increase_by_one();
                *tx_id_seq
            };
            let withdrawal_amout = Withdrawal(Amount::new(amount));
            
            let mut cb = cb_test_w.borrow_mut();
            cb.assert_ok_transaction_and_assert_unlocked_account(deposit_transaction_id, deposit_amout);
            cb.assert_ok_transaction_and_assert_unlocked_account(withdrawal_transaction_id, withdrawal_amout);
        });
        let available = cb_test_w.borrow().cb.available;
        let available: f64 = available.into();
        let available2: u64 = available as u64;
        assert_eq!(
            available2,
            *amount_count.borrow(),
            "available {}",
            available
        );
    }

    #[test]
    fn test_deposits_and_withdrawals_higher_amounts_only() {
        use IgnoredTransactionReason::*;
        use TransactionType::*;
        let cb_test_w = RefCell::new(ClientBalanceTestWrapper::new());
        let global_tx_id_seq = RefCell::new(TransactionID::default());
        let amount_count = RefCell::new(1);

        proptest!(|(amount in 1f64..1000.0)| {
            let mut amount_count = amount_count.borrow_mut();
            *amount_count += 1u64;
            let amount = amount.into();
            let deposit_transaction_id = {
                let mut tx_id_seq = global_tx_id_seq.borrow_mut();
                tx_id_seq.increase_by_one();
                *tx_id_seq
            };
            let deposit_amout = Deposit(amount);
            
            let withdrawal_transaction_id = {
                let mut tx_id_seq = global_tx_id_seq.borrow_mut();
                tx_id_seq.increase_by_one();
                *tx_id_seq
            };
            let withdrawal_amout = Withdrawal(amount);
            
            let mut cb = cb_test_w.borrow_mut();
            cb.assert_ok_transaction_and_assert_unlocked_account(deposit_transaction_id, deposit_amout);
            cb.assert_ok_transaction_and_assert_unlocked_account(withdrawal_transaction_id, withdrawal_amout);

            let withdrawal_transaction_id = {
                let mut tx_id_seq = global_tx_id_seq.borrow_mut();
                tx_id_seq.increase_by_one();
                *tx_id_seq
            };
            let withdrawal_amout = TransactionType::Withdrawal(amount);
            let ignored = cb.assert_err_transaction_ignored(withdrawal_transaction_id, withdrawal_amout);
            assert_eq!(ignored, InsufficientAvailableFunds);
        });
    }

    prop_compose! {
        fn deposit_or_withdraw() (
            amount in 1f64..10000f64,
            withdrawal in 0..1,
      ) -> TransactionType {
          if withdrawal == 1 {
              TransactionType::Withdrawal(amount.into())
          } else {
              TransactionType::Deposit(amount.into())
          }
      }
    }

    prop_compose! {
        fn deposits() (
            amount in 1f64..100f64,
      ) -> TransactionType {
        TransactionType::Deposit(amount.into())
      }
    }

    prop_compose! {
        fn withdrawals() (
            amount in 1f64..10000f64,
      ) -> TransactionType {
        TransactionType::Withdrawal(amount.into())
      }
    }

    fn no_chargebacks_strategy() -> BoxedStrategy<TransactionType> {
        prop_oneof![
            Just(TransactionType::Dispute),
            Just(TransactionType::Resolve),
            deposit_or_withdraw(),
        ]
        .boxed()
    }

    fn all_transactions_strategy() -> BoxedStrategy<TransactionType> {
        prop_oneof![
            Just(TransactionType::Chargeback),
            Just(TransactionType::Dispute),
            Just(TransactionType::Resolve),
            deposit_or_withdraw(),
        ]
        .boxed()
    }

    #[test]
    fn test_deposits_and_withdrawals_disputes_and_resolves() {
        use rand::{thread_rng, Rng};

        use IgnoredTransactionReason::*;
        use TransactionType::*;
        let cb_test_w = RefCell::new(ClientBalanceTestWrapper::new());
        let global_tx_id_seq = RefCell::new(TransactionID(1));
        let rng = RefCell::new(thread_rng());

        proptest!(|(transaction_type in no_chargebacks_strategy())| {
            let mut rng = rng.borrow_mut();
            let transaction_id =  match transaction_type {
                Deposit(_) => {
                    let mut tx_id_seq = global_tx_id_seq.borrow_mut();
                    let transaction_id = *tx_id_seq;
                    tx_id_seq.increase_by_one();
                    transaction_id
                },
                Withdrawal(_) => {
                    let mut tx_id_seq = global_tx_id_seq.borrow_mut();
                    let transaction_id = *tx_id_seq;
                    tx_id_seq.increase_by_one();
                    transaction_id
                },
                Dispute => {
                    let tx_id_seq = global_tx_id_seq.borrow();
                    let transaction_id = *tx_id_seq;
                    let r = rng.gen_range(0..transaction_id.0);
                    TransactionID(r)
                },
                Resolve => {
                    let tx_id_seq = global_tx_id_seq.borrow();
                    let transaction_id = *tx_id_seq;
                    let r = rng.gen_range(0..transaction_id.0);
                    TransactionID(r)
                },
                Chargeback => panic!("INVALID STRATEGY"),
            };
            
            let mut cb = cb_test_w.borrow_mut();
            let tx = Transaction {
                client_id: Default::default(),
                transaction_id,
                transaction_type
            };
            let res = cb.cb.handle_transaction(tx);
            if let Err(HandledTransactionError::InvalidClientBalance(_, _)) = res {
                panic!("Got invalid client balance");
            }
            cb.assert_unlocked_account();
        });
    }

    fn dispute_transactions_strategy() -> BoxedStrategy<TransactionType> {
        prop_oneof![deposit_or_withdraw(), Just(TransactionType::Dispute)].boxed()
    }

    #[test]
    fn test_deposits_and_withdrawals_disputes() {
        use rand::{thread_rng, Rng};

        use IgnoredTransactionReason::*;
        use TransactionType::*;
        let cb_test_w = RefCell::new(ClientBalanceTestWrapper::new());
        let global_tx_id_seq = RefCell::new(TransactionID(1));
        let rng = RefCell::new(thread_rng());

        proptest!(|(transaction_type in dispute_transactions_strategy())| {
            let mut rng = rng.borrow_mut();
            let transaction_id =  match transaction_type {
                Deposit(_) => {
                    let mut tx_id_seq = global_tx_id_seq.borrow_mut();
                    let transaction_id = *tx_id_seq;
                    tx_id_seq.increase_by_one();
                    transaction_id
                },
                Withdrawal(_) => {
                    let mut tx_id_seq = global_tx_id_seq.borrow_mut();
                    let transaction_id = *tx_id_seq;
                    tx_id_seq.increase_by_one();
                    transaction_id
                },
                Dispute => {
                    let tx_id_seq = global_tx_id_seq.borrow();
                    let transaction_id = *tx_id_seq;
                    let r = rng.gen_range(0..transaction_id.0);
                    TransactionID(r)
                },
                Resolve => panic!("INVALID STRATEGY"),
                Chargeback => panic!("INVALID STRATEGY"),
            };
            
            let mut cb = cb_test_w.borrow_mut();
            let tx = Transaction {
                client_id: Default::default(),
                transaction_id,
                transaction_type
            };
            let res = cb.cb.handle_transaction(tx);
            if let Err(HandledTransactionError::InvalidClientBalance(_, _)) = res {
                panic!("Got invalid client balance");
            }
            cb.assert_unlocked_account();
        });
    }

    #[test]
    fn test_deposits_disputes() {
        use IgnoredTransactionReason::*;
        use TransactionType::*;
        let cb_test_w = RefCell::new(ClientBalanceTestWrapper::new());
        let global_tx_id_seq = RefCell::new(TransactionID(1));
        let amount_sum = RefCell::new(Amount::new(0.0));

        proptest!(|(amount in 1f64..1000.0)| {
            let amount = amount.into();
            let mut amount_sum = amount_sum.borrow_mut();
            *amount_sum += amount;
            let transaction_id = {
                let mut tx_id_seq = global_tx_id_seq.borrow_mut();
                let transaction_id = *tx_id_seq;
                tx_id_seq.increase_by_one();
                transaction_id
            };
            let mut cb = cb_test_w.borrow_mut();
            cb.assert_ok_transaction_and_assert_unlocked_account(transaction_id, Deposit(amount));
            cb.assert_ok_transaction_and_assert_unlocked_account(transaction_id, Dispute);
        });
        let cb = cb_test_w.borrow();
        let amount_sum = amount_sum.borrow();
        assert_eq!(cb.cb.held, *amount_sum);
        assert!(cb.cb.available.is_zero());
        assert_eq!(cb.cb.total, *amount_sum);
    }

    #[test]
    fn test_deposits_disputes_resolves() {
        use IgnoredTransactionReason::*;
        use TransactionType::*;
        let cb_test_w = RefCell::new(ClientBalanceTestWrapper::new());
        let global_tx_id_seq = RefCell::new(TransactionID(1));
        let amount_sum = RefCell::new(Amount::new(0.0));

        proptest!(|(amount in 1f64..1000.0)| {
            let amount = amount.into();
            let mut amount_sum = amount_sum.borrow_mut();
            *amount_sum += amount;
            let transaction_id = {
                let mut tx_id_seq = global_tx_id_seq.borrow_mut();
                let transaction_id = *tx_id_seq;
                tx_id_seq.increase_by_one();
                transaction_id
            };
            let mut cb = cb_test_w.borrow_mut();
            cb.assert_ok_transaction_and_assert_unlocked_account(transaction_id, Deposit(amount));
            cb.assert_ok_transaction_and_assert_unlocked_account(transaction_id, Dispute);
            cb.assert_ok_transaction_and_assert_unlocked_account(transaction_id, Resolve);
        });
        let cb = cb_test_w.borrow();
        let amount_sum = amount_sum.borrow();
        assert_eq!(cb.cb.available, *amount_sum);
        assert!(cb.cb.held.is_zero());
        assert_eq!(cb.cb.total, *amount_sum);
    }

    #[test]
    fn test_deposits_withdrawal_disputes_chargeback_deposit() {
        use IgnoredTransactionReason::*;
        use TransactionType::*;
        let cb_test_w = RefCell::new(ClientBalanceTestWrapper::new());
        let global_tx_id_seq = RefCell::new(TransactionID(1));
        let amount_sum = RefCell::new(Amount::new(0.0));

        proptest!(|(amount in 1f64..1000.0)| {
            let amount = amount.into();
            let mut amount_sum = amount_sum.borrow_mut();
            *amount_sum += amount;
            let deposit_transaction_id = {
                let mut tx_id_seq = global_tx_id_seq.borrow_mut();
                let transaction_id = *tx_id_seq;
                tx_id_seq.increase_by_one();
                transaction_id
            };
            let withdrawal_transaction_id = {
                let mut tx_id_seq = global_tx_id_seq.borrow_mut();
                let transaction_id = *tx_id_seq;
                tx_id_seq.increase_by_one();
                transaction_id
            };
            let mut cb = cb_test_w.borrow_mut();
            cb.assert_ok_transaction_and_assert_unlocked_account(deposit_transaction_id, Deposit(amount));
            cb.assert_ok_transaction_and_assert_unlocked_account(withdrawal_transaction_id, Withdrawal(amount));
            cb.assert_ok_transaction_and_assert_unlocked_account(withdrawal_transaction_id, Dispute);
            cb.assert_ok_transaction_and_assert_unlocked_account(deposit_transaction_id, Dispute);
            
        });
        let mut cb = cb_test_w.borrow_mut();
        assert!(cb.cb.available.is_zero());
        assert!(cb.cb.held.is_zero());
        assert!(cb.cb.total.is_zero());
        let tx_id_seq = global_tx_id_seq.borrow();
        let transaction_id = *tx_id_seq;
        let transaction_id = TransactionID(transaction_id.0 - 1u32);
        cb.assert_ok_transaction_and_assert_frozen_account(transaction_id, Chargeback);
        assert!(cb.cb.available.is_zero());
        assert!(!cb.cb.held.is_negative());
        assert!(!cb.cb.total.is_negative());
    }

    #[test]
    fn test_deposits_withdrawal_disputes_chargeback_withdrawal() {
        use IgnoredTransactionReason::*;
        use TransactionType::*;
        let cb_test_w = RefCell::new(ClientBalanceTestWrapper::new());
        let global_tx_id_seq = RefCell::new(TransactionID(1));
        let amount_sum = RefCell::new(Amount::new(0.0));

        proptest!(|(amount in 1f64..1000.0)| {
            let amount = amount.into();
            let mut amount_sum = amount_sum.borrow_mut();
            *amount_sum += amount;
            let deposit_transaction_id = {
                let mut tx_id_seq = global_tx_id_seq.borrow_mut();
                let transaction_id = *tx_id_seq;
                tx_id_seq.increase_by_one();
                transaction_id
            };
            let withdrawal_transaction_id = {
                let mut tx_id_seq = global_tx_id_seq.borrow_mut();
                let transaction_id = *tx_id_seq;
                tx_id_seq.increase_by_one();
                transaction_id
            };
            let mut cb = cb_test_w.borrow_mut();
            cb.assert_ok_transaction_and_assert_unlocked_account(deposit_transaction_id, Deposit(amount));
            cb.assert_ok_transaction_and_assert_unlocked_account(withdrawal_transaction_id, Withdrawal(amount));
            cb.assert_ok_transaction_and_assert_unlocked_account(withdrawal_transaction_id, Dispute);
            cb.assert_ok_transaction_and_assert_unlocked_account(deposit_transaction_id, Dispute);
            
        });
        let mut cb = cb_test_w.borrow_mut();
        assert!(cb.cb.available.is_zero());
        assert!(cb.cb.held.is_zero());
        assert!(cb.cb.total.is_zero());
        let tx_id_seq = global_tx_id_seq.borrow();
        let transaction_id = *tx_id_seq;
        let transaction_id = TransactionID(transaction_id.0 - 2u32);
        cb.assert_ok_transaction_and_assert_frozen_account(transaction_id, Chargeback);
        assert!(cb.cb.available.is_zero());
        assert!(cb.cb.held.is_negative());
        assert!(cb.cb.total.is_negative());
    }


    // // from here on these are not really tests for corectness 
    // macro_rules! print_struct_size
    // {
    //     ($struct_name:ident) =>
    //     {
    //         println!("{}: is {}", stringify!($struct_name), std::mem::size_of::<$struct_name>());
    //     };
    // }
    // #[test]
    // fn print_memory_usages_for_experimental() {
    //     print_struct_size!(ClientID);
    //     print_struct_size!(TransactionID);
    //     print_struct_size!(TransactionState);
    //     print_struct_size!(CreditDebitState);
    //     print_struct_size!(ClientBalance);
    //     print_struct_size!(Atm);
    // }

}
