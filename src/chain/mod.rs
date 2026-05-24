#![allow(dead_code)]

use anyhow::anyhow;
use revm::{
    db::{CacheDB, EmptyDB},
    inspector_handle_register,
    primitives::{
        AccountInfo, Address, Bytes, ExecutionResult, Output, TxKind, U256,
    },
    DatabaseCommit, Evm,
};

use crate::fuzzing::coverage::{CoverageInspector, LocalCoverage};

// ── public data types ────────────────────────────────────────────────────────

/// Result of a contract call executed on the TestChain.
pub struct CallResult {
    pub success: bool,
    pub output: Bytes,
    pub gas_used: u64,
    /// Raw ABI-encoded revert payload when the call failed, or `None` on success.
    pub revert_reason: Option<Bytes>,
}

/// Opaque snapshot of chain state that can be restored later.
#[derive(Clone)]
pub struct ChainSnapshot {
    db: CacheDB<EmptyDB>,
    block_number: u64,
    block_timestamp: u64,
}

// ── TestChain ────────────────────────────────────────────────────────────────

/// In-process EVM chain backed by revm's `CacheDB<EmptyDB>`.
#[derive(Clone)]
pub struct TestChain {
    db: CacheDB<EmptyDB>,
    block_number: u64,
    block_timestamp: u64,
}

impl Default for TestChain {
    fn default() -> Self {
        Self::new()
    }
}

impl TestChain {
    /// Create a fresh chain with default block state.
    pub fn new() -> Self {
        Self {
            db: CacheDB::new(EmptyDB::default()),
            block_number: 1,
            block_timestamp: 1_000_000,
        }
    }

    // ── helpers ──────────────────────────────────────────────────────────

    /// Ensure an address exists in the DB with sufficient balance so that the
    /// EVM's balance check passes.  The very large balance is enough for any
    /// practical gas cost * gas_limit product.
    fn ensure_funded(&mut self, addr: Address) {
        let entry = self.db.accounts.entry(addr).or_default();
        if entry.info.balance < U256::from(u128::MAX) {
            entry.info.balance = U256::from(u128::MAX);
        }
    }

    /// Build an EVM instance with the current DB and block environment.
    ///
    /// We take the DB by value to give it to the EVM; the caller must put it
    /// back via `Self::commit_evm` afterwards.
    fn take_evm(&mut self) -> Evm<'static, (), CacheDB<EmptyDB>> {
        // Swap the DB out so we can give ownership to the Evm.
        let db = std::mem::replace(&mut self.db, CacheDB::new(EmptyDB::default()));
        let block_number = self.block_number;
        let block_timestamp = self.block_timestamp;

        Evm::builder()
            .with_db(db)
            .modify_block_env(|b| {
                b.number = U256::from(block_number);
                b.timestamp = U256::from(block_timestamp);
                b.gas_limit = U256::from(u64::MAX);
                b.basefee = U256::ZERO;
            })
            .modify_tx_env(|tx| {
                tx.gas_limit = u64::MAX / 2;
                tx.gas_price = U256::ZERO;
            })
            .build()
    }

    /// Consume a finished EVM and reclaim its DB.
    fn return_evm(&mut self, evm: Evm<'static, (), CacheDB<EmptyDB>>) {
        self.db = evm.into_context().evm.inner.db;
    }

    // ── public API ───────────────────────────────────────────────────────

    /// Deploy `bytecode` from `deployer`, sending `value` wei.
    ///
    /// Returns the address of the newly-created contract.
    pub fn deploy(
        &mut self,
        deployer: Address,
        bytecode: Bytes,
        value: U256,
    ) -> anyhow::Result<Address> {
        self.ensure_funded(deployer);
        let mut evm = self.take_evm();

        evm.tx_mut().caller = deployer;
        evm.tx_mut().transact_to = TxKind::Create;
        evm.tx_mut().data = bytecode;
        evm.tx_mut().value = value;

        let result_and_state = evm
            .transact()
            .map_err(|e| anyhow!("deploy EVM error: {e:?}"))?;

        let result = result_and_state.result;
        // Commit state changes into the DB, then reclaim the DB.
        evm.context.evm.db.commit(result_and_state.state);
        self.return_evm(evm);

        match result {
            ExecutionResult::Success {
                output: Output::Create(_, Some(addr)),
                ..
            } => Ok(addr),
            ExecutionResult::Success {
                output: Output::Create(_, None),
                ..
            } => Err(anyhow!("deployment succeeded but no address returned")),
            ExecutionResult::Revert { output, .. } => {
                Err(anyhow!("deploy reverted: 0x{}", hex::encode(&output)))
            }
            ExecutionResult::Halt { reason, .. } => {
                Err(anyhow!("deploy halted: {reason:?}"))
            }
            _ => Err(anyhow!("unexpected execution result")),
        }
    }

    /// Call a deployed contract at `to` from `from` with `calldata` and `value`.
    pub fn call(
        &mut self,
        from: Address,
        to: Address,
        calldata: Bytes,
        value: U256,
    ) -> anyhow::Result<CallResult> {
        self.ensure_funded(from);
        let mut evm = self.take_evm();

        evm.tx_mut().caller = from;
        evm.tx_mut().transact_to = TxKind::Call(to);
        evm.tx_mut().data = calldata;
        evm.tx_mut().value = value;

        let result_and_state = evm
            .transact()
            .map_err(|e| anyhow!("call EVM error: {e:?}"))?;

        let result = result_and_state.result;
        evm.context.evm.db.commit(result_and_state.state);
        self.return_evm(evm);

        let call_result = match result {
            ExecutionResult::Success {
                output: Output::Call(bytes),
                gas_used,
                ..
            } => CallResult {
                success: true,
                output: bytes,
                gas_used,
                revert_reason: None,
            },
            ExecutionResult::Success { gas_used, .. } => CallResult {
                success: true,
                output: Bytes::new(),
                gas_used,
                revert_reason: None,
            },
            ExecutionResult::Revert { output, gas_used } => CallResult {
                success: false,
                output: Bytes::new(),
                gas_used,
                revert_reason: Some(output),
            },
            ExecutionResult::Halt { gas_used, .. } => CallResult {
                success: false,
                output: Bytes::new(),
                gas_used,
                revert_reason: None,
            },
        };

        Ok(call_result)
    }

    /// Like `call()` but also collects coverage via `CoverageInspector`.
    ///
    /// Returns `(CallResult, LocalCoverage)`.
    pub fn call_with_coverage(
        &mut self,
        from: Address,
        to: Address,
        calldata: Bytes,
        value: U256,
    ) -> anyhow::Result<(CallResult, LocalCoverage)> {
        self.ensure_funded(from);

        // Swap the DB out so we can give ownership to the Evm.
        let db = std::mem::replace(&mut self.db, CacheDB::new(EmptyDB::default()));
        let block_number = self.block_number;
        let block_timestamp = self.block_timestamp;

        let mut inspector = CoverageInspector::new();

        let mut evm = Evm::builder()
            .with_db(db)
            .with_external_context(&mut inspector)
            .modify_block_env(|b| {
                b.number = U256::from(block_number);
                b.timestamp = U256::from(block_timestamp);
                b.gas_limit = U256::from(u64::MAX);
                b.basefee = U256::ZERO;
            })
            .modify_tx_env(|tx| {
                tx.gas_limit = u64::MAX / 2;
                tx.gas_price = U256::ZERO;
                tx.caller = from;
                tx.transact_to = TxKind::Call(to);
                tx.data = calldata;
                tx.value = value;
            })
            .append_handler_register(inspector_handle_register)
            .build();

        let result_and_state = evm
            .transact()
            .map_err(|e| anyhow!("call_with_coverage EVM error: {e:?}"))?;

        let result = result_and_state.result;
        evm.context.evm.db.commit(result_and_state.state);

        // Reclaim the DB.
        self.db = evm.into_context().evm.inner.db;

        let coverage = inspector.take_coverage();

        let call_result = match result {
            ExecutionResult::Success {
                output: Output::Call(bytes),
                gas_used,
                ..
            } => CallResult {
                success: true,
                output: bytes,
                gas_used,
                revert_reason: None,
            },
            ExecutionResult::Success { gas_used, .. } => CallResult {
                success: true,
                output: Bytes::new(),
                gas_used,
                revert_reason: None,
            },
            ExecutionResult::Revert { output, gas_used } => CallResult {
                success: false,
                output: Bytes::new(),
                gas_used,
                revert_reason: Some(output),
            },
            ExecutionResult::Halt { gas_used, .. } => CallResult {
                success: false,
                output: Bytes::new(),
                gas_used,
                revert_reason: None,
            },
        };

        Ok((call_result, coverage))
    }

    /// Save the current chain state and return a handle to it.
    pub fn snapshot(&self) -> ChainSnapshot {
        ChainSnapshot {
            db: self.db.clone(),
            block_number: self.block_number,
            block_timestamp: self.block_timestamp,
        }
    }

    /// Restore the chain to a previously saved snapshot.
    pub fn restore(&mut self, snapshot: ChainSnapshot) {
        self.db = snapshot.db;
        self.block_number = snapshot.block_number;
        self.block_timestamp = snapshot.block_timestamp;
    }

    /// Set the current block number.
    pub fn set_block_number(&mut self, n: u64) {
        self.block_number = n;
    }

    /// Set the current block timestamp (Unix seconds).
    pub fn set_block_timestamp(&mut self, ts: u64) {
        self.block_timestamp = ts;
    }

    /// Return the ETH balance of `addr`.
    pub fn get_balance(&self, addr: Address) -> U256 {
        self.db
            .accounts
            .get(&addr)
            .map(|acc| acc.info.balance)
            .unwrap_or(U256::ZERO)
    }

    /// Give `addr` exactly `amount` wei (overwrites existing balance).
    pub fn fund(&mut self, addr: Address, amount: U256) {
        let info = self
            .db
            .accounts
            .entry(addr)
            .or_default()
            .info
            .clone();

        self.db.insert_account_info(
            addr,
            AccountInfo {
                balance: amount,
                nonce: info.nonce,
                code_hash: info.code_hash,
                code: info.code,
            },
        );
    }
}
