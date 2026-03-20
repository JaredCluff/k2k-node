use crate::config::K2KNodeConfig;

pub async fn run(config: K2KNodeConfig) -> anyhow::Result<()> {
    let url = format!("http://{}:{}/k2k/v1/health", config.host, config.port);

    let client = reqwest::Client::new();
    match client.get(&url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                let body: serde_json::Value = resp.json().await?;
                println!("{}", serde_json::to_string_pretty(&body)?);
            } else {
                println!("Node returned status: {}", resp.status());
            }
        }
        Err(e) => {
            println!("Node is not running or unreachable: {}", e);
            println!("  Expected at: {}", url);
        }
    }

    Ok(())
}
