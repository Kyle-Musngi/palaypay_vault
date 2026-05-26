#![cfg(test)]
extern crate std;

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};
use soroban_sdk::token::Client as TokenClient;
use soroban_sdk::token::StellarAssetClient;

fn setup_env() -> (Env, Address, Address, Address, Address, PalayPayVaultClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let coop_admin = Address::generate(&env);
    let farmer = Address::generate(&env);
    let random_user = Address::generate(&env);
    
    // Setup Mock Token (representing PHPC / USDC)
    let token_admin = Address::generate(&env);
    let token_contract = env.register_stellar_asset_contract(token_admin.clone());
    let token_client = TokenClient::new(&env, &token_contract);
    let token_admin_client = StellarAssetClient::new(&env, &token_contract);
    
    // Mint 50,000 tokens to the cooperative admin to fund the harvest season
    token_admin_client.mint(&coop_admin, &50_000);

    let contract_id = env.register_contract(None, PalayPayVault);
    let vault_client = PalayPayVaultClient::new(&env, &contract_id);

    (env, coop_admin, farmer, random_user, token_contract, vault_client)
}

mod tests {
    use super::*;

    // Test 1 (Happy path): Admin funds treasury and pays out a farmer successfully.
    #[test]
    fn test_successful_farmer_payout() {
        let (env, admin, farmer, _random, token, vault) = setup_env();
        let token_client = TokenClient::new(&env, &token);

        // Init with price = 25 PHPC per kilo
        vault.init(&admin, &token, &25);
        vault.fund_treasury(&10_000); // Admin deposits 10k into the contract
        
        // Farmer delivers 100 kilos (Expected payout: 2500)
        vault.payout_farmer(&farmer, &100);

        assert_eq!(token_client.balance(&farmer), 2500);
        assert_eq!(token_client.balance(&vault.address), 7500); // 10k - 2.5k
    }

    // Test 2 (Edge case): Unauthorized caller attempts to trigger a farmer payout.
    #[test]
    #[should_panic(expected = "not authorized")]
    fn test_unauthorized_payout_trigger() {
        let (env, admin, farmer, random_user, token, vault) = setup_env();

        vault.init(&admin, &token, &25);
        vault.fund_treasury(&10_000);

        // Mock auth to represent a random user (or hacker) trying to call the payout function
        env.mock_auths(&[
            soroban_sdk::testutils::MockAuth {
                address: &random_user,
                invoke: &soroban_sdk::testutils::MockAuthInvoke {
                    contract: &vault.address,
                    fn_name: "payout_farmer",
                    args: (farmer.clone(), 100i128).into_val(&env),
                    sub_invokes: &[],
                },
            }
        ]);
        
        vault.payout_farmer(&farmer, &100);
    }

    // Test 3 (State verification): Admin updates the market price, and payout reflects the change.
    #[test]
    fn test_dynamic_price_update() {
        let (env, admin, farmer, _random, token, vault) = setup_env();
        let token_client = TokenClient::new(&env, &token);

        vault.init(&admin, &token, &25);
        vault.fund_treasury(&10_000);
        
        // Market price of rice drops to 20
        vault.update_price(&20);
        
        // Farmer delivers 100 kilos (Expected payout: 2000, not 2500)
        vault.payout_farmer(&farmer, &100);

        assert_eq!(token_client.balance(&farmer), 2000);
        
        // Verify internal state updated correctly
        env.as_contract(&vault.address, || {
            let config: CoopConfig = env.storage().instance().get(&DataKey::CoopConfig).unwrap();
            assert_eq!(config.price_per_kilo, 20);
        });
    }

    // Test 4 (Edge case): Payout exceeds the available treasury balance.
    #[test]
    #[should_panic(expected = "Insufficient cooperative treasury funds for this payout")]
    fn test_insufficient_treasury_balance() {
        let (env, admin, farmer, _random, token, vault) = setup_env();

        vault.init(&admin, &token, &25);
        vault.fund_treasury(&1000); // Coop only has 1000 in the vault
        
        // Farmer delivers a massive harvest of 100 kilos (Requires 2500, but vault only has 1000)
        vault.payout_farmer(&farmer, &100); // Should panic here
    }

    // Test 5 (State verification): Ensure treasury balance accurately tracks multiple payouts.
    #[test]
    fn test_multiple_payouts_state() {
        let (env, admin, farmer, _random, token, vault) = setup_env();
        let token_client = TokenClient::new(&env, &token);

        vault.init(&admin, &token, &25);
        vault.fund_treasury(&5000);
        
        vault.payout_farmer(&farmer, &10); // 250 payout
        vault.payout_farmer(&farmer, &20); // 500 payout
        
        assert_eq!(token_client.balance(&farmer), 750);
        assert_eq!(token_client.balance(&vault.address), 4250); // 5000 - 750
    }
}