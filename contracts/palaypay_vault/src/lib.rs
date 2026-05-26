#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, token};

#[contracttype]
pub enum DataKey {
    CoopConfig,
}

#[contracttype]
#[derive(Clone)]
pub struct CoopConfig {
    pub admin: Address,
    pub token: Address,
    pub price_per_kilo: i128,
}

#[contract]
pub struct PalayPayVault;

#[contractimpl]
impl PalayPayVault {
    /// Initializes the agricultural cooperative's treasury with an admin, payment token, and current palay price.
    pub fn init(env: Env, admin: Address, token: Address, price_per_kilo: i128) {
        admin.require_auth();
        
        if price_per_kilo <= 0 {
            panic!("Price per kilo must be greater than 0");
        }

        let config = CoopConfig {
            admin,
            token,
            price_per_kilo,
        };
        env.storage().instance().set(&DataKey::CoopConfig, &config);
    }

    /// Allows the cooperative to fund the smart contract treasury for the harvest season.
    pub fn fund_treasury(env: Env, amount: i128) {
        let config: CoopConfig = env.storage().instance().get(&DataKey::CoopConfig).unwrap();
        config.admin.require_auth();

        let token_client = token::Client::new(&env, &config.token);
        token_client.transfer(
            &config.admin,
            &env.current_contract_address(),
            &amount,
        );
    }

    /// Allows the admin to update the market price per kilo of unhusked rice (palay).
    pub fn update_price(env: Env, new_price: i128) {
        let mut config: CoopConfig = env.storage().instance().get(&DataKey::CoopConfig).unwrap();
        config.admin.require_auth();

        if new_price <= 0 {
            panic!("New price must be greater than 0");
        }

        config.price_per_kilo = new_price;
        env.storage().instance().set(&DataKey::CoopConfig, &config);
    }

    /// Disburses payment instantly to the farmer based on the kilos delivered.
    pub fn payout_farmer(env: Env, farmer: Address, kilos_delivered: i128) {
        let config: CoopConfig = env.storage().instance().get(&DataKey::CoopConfig).unwrap();
        config.admin.require_auth(); // Only the warehouse admin can trigger a verified payout

        if kilos_delivered <= 0 {
            panic!("Kilos delivered must be greater than 0");
        }

        let total_payout = kilos_delivered * config.price_per_kilo;
        let token_client = token::Client::new(&env, &config.token);
        
        let treasury_balance = token_client.balance(&env.current_contract_address());
        if total_payout > treasury_balance {
            panic!("Insufficient cooperative treasury funds for this payout");
        }

        // Transfer funds to the farmer
        token_client.transfer(
            &env.current_contract_address(),
            &farmer,
            &total_payout,
        );
    }
}