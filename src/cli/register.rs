use crate::config::K2KNodeConfig;
use crate::keys::KeyManager;

pub async fn run(config: K2KNodeConfig, peer_url: &str) -> anyhow::Result<()> {
    std::fs::create_dir_all(&config.data_dir)?;

    let key_manager = KeyManager::load_or_generate(&config.keys_dir())?;
    let public_key_pem = key_manager.public_key_pem().to_string();

    let client_id = config.node_id.clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let client_name = config.node_name.clone();

    let body = serde_json::json!({
        "client_id": client_id,
        "client_name": client_name,
        "public_key_pem": public_key_pem,
    });

    let url = format!("{}/k2k/v1/register-client", peer_url.trim_end_matches('/'));
    println!("Registering with {}...", url);

    let client = reqwest::Client::new();
    let resp = client.post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    if resp.status().is_success() {
        let result: serde_json::Value = resp.json().await?;
        println!("Registration result:");
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        let status = resp.status();
        let body = resp.text().await?;
        anyhow::bail!("Registration failed ({}): {}", status, body);
    }

    Ok(())
}
