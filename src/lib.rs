#![no_std]
extern crate alloc;

use alloc::vec::Vec;
use stylus_sdk::{
    alloy_primitives::{Address, U256},
    alloy_sol_types::sol,
    evm, msg,
    prelude::*,
};

sol! {
    event ContributionDeposited(address indexed beneficiary, uint256 amount, uint64 timestamp);
    event BeneficiaryDesignated(address indexed owner, address indexed beneficiary, uint16 allocationBps);
    event PensionDistributed(address indexed beneficiary, uint256 principal, uint256 yieldAmount);
    error Unauthorized();
    error VestingCliffNotMet(uint64 currentTime, uint64 cliffTime);
    error InvalidBeneficiaryAllocation();
    error EmergencyFreezeActive();
}

#[storage]
#[entrypoint]
pub struct PensionQuantumVault {
    owner: StorageAddress,
    emergency_multisig: StorageAddress,
    is_frozen: StorageBool,
    lockup_end_timestamp: StorageU64,
    cliff_timestamp: StorageU64,
    total_principal: StorageU256,
}

#[public]
impl PensionQuantumVault {
    pub fn init(&mut self, emergency_multisig: Address, lockup_duration: u64, cliff_duration: u64) -> Result<(), Vec<u8>> {
        let current_time = evm::block_timestamp();
        self.owner.set(msg::sender());
        self.emergency_multisig.set(emergency_multisig);
        self.is_frozen.set(false);
        self.lockup_end_timestamp.set(current_time + lockup_duration);
        self.cliff_timestamp.set(current_time + cliff_duration);
        self.total_principal.set(U256::ZERO);
        Ok(())
    }

    pub fn contribute(&mut self) -> Result<(), Vec<u8>> {
        if self.is_frozen.get() {
            return Err(EmergencyFreezeActive {}.into());
        }
        let deposit = msg::value();
        let current_total = self.total_principal.get();
        self.total_principal.set(current_total + deposit);

        evm::log(ContributionDeposited {
            beneficiary: msg::sender(),
            amount: deposit,
            timestamp: evm::block_timestamp(),
        });
        Ok(())
    }

    pub fn execute_distribution(&mut self, recipient: Address) -> Result<U256, Vec<u8>> {
        if msg::sender() != self.owner.get() {
            return Err(Unauthorized {}.into());
        }
        let now = evm::block_timestamp();
        if now < self.cliff_timestamp.get() {
            return Err(VestingCliffNotMet {
                currentTime: now,
                cliffTime: self.cliff_timestamp.get(),
            }.into());
        }

        let principal = self.total_principal.get();
        self.total_principal.set(U256::ZERO);

        stylus_sdk::call::transfer_eth(recipient, principal)?;

        evm::log(PensionDistributed {
            beneficiary: recipient,
            principal,
            yieldAmount: U256::ZERO,
        });

        Ok(principal)
    }

    pub fn emergency_freeze(&mut self) -> Result<(), Vec<u8>> {
        if msg::sender() != self.emergency_multisig.get() {
            return Err(Unauthorized {}.into());
        }
        self.is_frozen.set(true);
        Ok(())
    }
}
