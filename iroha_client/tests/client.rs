#[cfg(test)]
mod tests {
    use futures::executor;
    use iroha::{
        account::isi::{CreateAccount, CreateRole},
        asset::isi::{AddAssetQuantity, TransferAsset},
        domain::isi::CreateDomain,
        prelude::*,
    };
    use iroha_client::client::{self, Client};
    use std::thread;
    use tempfile::TempDir;

    const CONFIGURATION_PATH: &str = "config.json";

    #[async_std::test]
    //TODO: use cucumber to write `gherkin` instead of code.
    async fn client_can_transfer_asset_to_another_account() {
        // Given
        thread::spawn(|| executor::block_on(create_and_start_iroha()));
        thread::sleep(std::time::Duration::from_millis(200));
        let create_role = CreateRole {
            role_name: "user".to_string(),
            permissions: Vec::new(),
        };
        let create_domain = CreateDomain {
            domain_name: "domain".to_string(),
            default_role: "user".to_string(),
        };
        let account1_id = Id::new("account1", "domain");
        let account2_id = Id::new("account2", "domain");
        let create_account1 = CreateAccount {
            account_id: account1_id.clone(),
            domain_name: "domain".to_string(),
            public_key: [63; 32],
        };
        let create_account2 = CreateAccount {
            account_id: account2_id.clone(),
            domain_name: "domain".to_string(),
            public_key: [63; 32],
        };
        let asset_id = Id::new("xor", "domain");
        let create_asset = AddAssetQuantity {
            asset_id: asset_id.clone(),
            account_id: account1_id.clone(),
            amount: 100,
        };
        let transfer_asset = TransferAsset {
            source_account_id: account1_id.clone(),
            destination_account_id: account2_id.clone(),
            asset_id: asset_id.clone(),
            description: "description".to_string(),
            amount: 20,
        };
        let mut iroha_client = Client::new(
            Configuration::from_path(CONFIGURATION_PATH).expect("Failed to load configuration."),
        );
        iroha_client
            .submit(create_role.into())
            .expect("Failed to create role.");
        iroha_client
            .submit(create_domain.into())
            .expect("Failed to create domain.");
        iroha_client
            .submit(create_account1.into())
            .expect("Failed to create account1.");
        iroha_client
            .submit(create_account2.into())
            .expect("Failed to create accoun2.");
        iroha_client
            .submit(create_asset.into())
            .expect("Failed to create asset.");
        //When
        iroha_client
            .submit(transfer_asset.into())
            .expect("Failed to submit command.");
        //Then
        let request = client::assets::by_account_id(account2_id);
        let query_result = iroha_client
            .request(&request)
            .expect("Failed to execute request.");
        let QueryResult::GetAccountAssets(result) = query_result;
        assert!(!result.assets.is_empty());
    }

    #[async_std::test]
    //TODO: use cucumber to write `gherkin` instead of code.
    async fn client_add_asset_quantity_to_existing_asset_should_increase_asset_amount() {
        // Given
        thread::spawn(|| executor::block_on(create_and_start_iroha()));
        thread::sleep(std::time::Duration::from_millis(200));
        let create_role = CreateRole {
            role_name: "user".to_string(),
            permissions: Vec::new(),
        };
        let create_domain = CreateDomain {
            domain_name: "domain".to_string(),
            default_role: "user".to_string(),
        };
        let account_id = Id::new("account", "domain");
        let create_account = CreateAccount {
            account_id: account_id.clone(),
            domain_name: "domain".to_string(),
            public_key: [63; 32],
        };
        let asset_id = Id::new("xor", "domain");
        let create_asset = AddAssetQuantity {
            asset_id: asset_id.clone(),
            account_id: account_id.clone(),
            amount: 100,
        };
        let mut iroha_client = Client::new(
            Configuration::from_path("config.json").expect("Failed to load configuration."),
        );
        iroha_client
            .submit(create_role.into())
            .expect("Failed to create role.");
        iroha_client
            .submit(create_domain.into())
            .expect("Failed to create domain.");
        iroha_client
            .submit(create_account.into())
            .expect("Failed to create account.");
        iroha_client
            .submit(create_asset.into())
            .expect("Failed to create asset.");
        //When
        let add_amount = 100;
        let add_asset_quantity = AddAssetQuantity {
            asset_id: asset_id.clone(),
            account_id: account_id.clone(),
            amount: add_amount,
        };
        iroha_client
            .submit(add_asset_quantity.into())
            .expect("Failed to create asset.");
        //Then
        let request = client::assets::by_account_id(account_id);
        let query_result = iroha_client
            .request(&request)
            .expect("Failed to execute request.");
        let QueryResult::GetAccountAssets(result) = query_result;
        assert!(!result.assets.is_empty());
        assert_eq!(
            add_amount,
            result.assets.first().expect("Asset should exist.").amount,
        );
    }

    async fn create_and_start_iroha() {
        println!("Iroha create.");
        let temp_dir = TempDir::new().expect("Failed to create TempDir.");
        let mut configuration =
            Configuration::from_path(CONFIGURATION_PATH).expect("Failed to load configuration.");
        configuration.kura_block_store_path(temp_dir.path());
        let mut iroha = Iroha::new(configuration);
        println!("Iroha start.");
        iroha.start().await.expect("Failed to start Iroha.");
        println!("Iroha started.");
    }
}
