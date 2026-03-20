use crate::config::K2KNodeConfig;
use crate::keys::KeyManager;

pub async fn run(config: K2KNodeConfig, peer_url: &str, query: &str, top_k: usize) -> anyhow::Result<()> {
    std::fs::create_dir_all(&config.data_dir)?;

    let key_manager = KeyManager::load_or_generate(&config.keys_dir())?;

    let k2k_client = k2k_common::K2KClient::new(
        key_manager.private_key_pem(),
        config.node_id.as_deref().unwrap_or("k2k-node"),
    )?;

    println!("Querying {} for: \"{}\"", peer_url, query);

    let response = k2k_client.query(
        peer_url.trim_end_matches('/'),
        query,
        config.node_id.as_deref().unwrap_or("k2k-node"),
        top_k,
        None,
    ).await?;

    println!("\nResults ({} found in {}ms):", response.total_results, response.query_time_ms);
    println!("{}", "-".repeat(60));

    for (i, result) in response.results.iter().enumerate() {
        println!(
            "\n{}. {} (score: {:.3})",
            i + 1,
            result.title,
            result.confidence
        );
        if let Some(path) = result.metadata.get("path").and_then(|v| v.as_str()) {
            println!("   Path: {}", path);
        }
        // Show first 200 chars of content
        let preview: String = result.content.chars().take(200).collect();
        println!("   {}", preview);
    }

    Ok(())
}
