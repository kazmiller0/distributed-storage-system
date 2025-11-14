use client::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager_addr = "http://[::1]:50051".to_string();
    let client = Client::new(manager_addr);

    // Example usage
    println!("=== Testing Put File ===");
    client
        .put_file(
            "file1".to_string(),
            vec![
                "rust".to_string(),
                "distributed".to_string(),
                "storage".to_string(),
            ],
        )
        .await?;

    println!("\n=== Testing Query ===");
    client.query_by_keyword("rust".to_string()).await?;

    println!("\n=== Testing Update ===");
    client
        .update_file(
            "file1".to_string(),
            vec!["storage".to_string()],
            vec!["database".to_string()],
        )
        .await?;

    println!("\n=== Testing Delete ===");
    client
        .delete_file(
            "file1".to_string(),
            vec![
                "rust".to_string(),
                "distributed".to_string(),
                "database".to_string(),
            ],
        )
        .await?;

    Ok(())
}
