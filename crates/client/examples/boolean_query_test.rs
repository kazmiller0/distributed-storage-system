use common::rpc::{
    manager_service_client::ManagerServiceClient, AddRequest, DeleteRequest, QueryRequest,
};

// Client structure
pub struct Client {
    manager_addr: String,
}

impl Client {
    pub fn new(manager_addr: String) -> Self {
        Client { manager_addr }
    }

    // Put file: add (fid, keywords) to the system
    pub async fn put_file(
        &self,
        fid: String,
        keywords: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut client = ManagerServiceClient::connect(self.manager_addr.clone()).await?;

        let request = AddRequest { fid, keywords };

        let response = client.add(request).await?;
        let resp = response.into_inner();

        if resp.success {
            println!("✅ Put file succeeded: {}", resp.message);
        } else {
            println!("❌ Put file failed: {}", resp.message);
        }

        Ok(())
    }

    // Query by keyword
    pub async fn query_by_keyword(
        &self,
        keyword: String,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut client = ManagerServiceClient::connect(self.manager_addr.clone()).await?;

        let request = QueryRequest {
            query_type: Some(common::rpc::query_request::QueryType::Keyword(
                keyword.clone(),
            )),
        };

        let response = client.query(request).await?;
        let resp = response.into_inner();

        if resp.verified {
            println!(
                "✅ Query '{}' succeeded, found {} files:",
                keyword,
                resp.fids.len()
            );
            for fid in &resp.fids {
                println!("     - {}", fid);
            }
        } else {
            println!("❌ Query verification failed!");
        }

        Ok(resp.fids)
    }

    // Query by boolean function
    pub async fn query_by_func(
        &self,
        boolean_func: String,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut client = ManagerServiceClient::connect(self.manager_addr.clone()).await?;

        let request = QueryRequest {
            query_type: Some(common::rpc::query_request::QueryType::BooleanFunction(
                boolean_func.clone(),
            )),
        };

        let response = client.query(request).await?;
        let resp = response.into_inner();

        if resp.verified {
            println!(
                "✅ Boolean query '{}' succeeded, found {} files:",
                boolean_func,
                resp.fids.len()
            );
            for fid in &resp.fids {
                println!("     - {}", fid);
            }
        } else {
            println!("❌ Query verification failed!");
        }

        Ok(resp.fids)
    }

    // Delete file: remove (fid, keywords) from the system
    pub async fn delete_file(
        &self,
        fid: String,
        keywords: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut client = ManagerServiceClient::connect(self.manager_addr.clone()).await?;

        let request = DeleteRequest { fid, keywords };

        let response = client.delete(request).await?;
        let resp = response.into_inner();

        if resp.success {
            println!("✅ Delete file succeeded: {}", resp.message);
        } else {
            println!("❌ Delete file failed: {}", resp.message);
        }

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager_addr = "http://[::1]:50051".to_string();
    let client = Client::new(manager_addr);

    println!("=== 布尔查询功能测试 ===\n");

    // 准备测试数据
    println!("📁 准备测试数据...\n");

    client
        .put_file(
            "file1".to_string(),
            vec!["rust".to_string(), "distributed".to_string()],
        )
        .await?;

    client
        .put_file(
            "file2".to_string(),
            vec!["rust".to_string(), "storage".to_string()],
        )
        .await?;

    client
        .put_file(
            "file3".to_string(),
            vec!["python".to_string(), "storage".to_string()],
        )
        .await?;

    client
        .put_file(
            "file4".to_string(),
            vec![
                "rust".to_string(),
                "storage".to_string(),
                "distributed".to_string(),
            ],
        )
        .await?;

    println!("\n");

    // 测试单关键词查询
    println!("=== 测试 1: 单关键词查询 ===\n");

    println!("查询: rust");
    client.query_by_keyword("rust".to_string()).await?;

    println!("\n查询: storage");
    client.query_by_keyword("storage".to_string()).await?;

    println!("\n查询: python");
    client.query_by_keyword("python".to_string()).await?;

    // 测试 AND 查询
    println!("\n=== 测试 2: AND 查询 ===\n");

    println!("查询: rust AND storage");
    let result = client.query_by_func("rust AND storage".to_string()).await?;
    println!("     预期: file2, file4");
    println!("     实际: {:?}\n", result);

    println!("查询: rust AND distributed");
    let result = client
        .query_by_func("rust AND distributed".to_string())
        .await?;
    println!("     预期: file1, file4");
    println!("     实际: {:?}\n", result);

    // 测试 OR 查询
    println!("\n=== 测试 3: OR 查询 ===\n");

    println!("查询: rust OR python");
    let result = client.query_by_func("rust OR python".to_string()).await?;
    println!("     预期: file1, file2, file3, file4");
    println!("     实际: {:?}\n", result);

    println!("查询: distributed OR python");
    let result = client
        .query_by_func("distributed OR python".to_string())
        .await?;
    println!("     预期: file1, file3, file4");
    println!("     实际: {:?}\n", result);

    // 测试复杂查询
    println!("\n=== 测试 4: 复杂布尔查询 ===\n");

    println!("查询: (rust OR python) AND storage");
    let result = client
        .query_by_func("(rust OR python) AND storage".to_string())
        .await?;
    println!("     预期: file2, file3, file4");
    println!("     实际: {:?}\n", result);

    println!("查询: rust AND (storage OR distributed)");
    let result = client
        .query_by_func("rust AND (storage OR distributed)".to_string())
        .await?;
    println!("     预期: file1, file2, file4");
    println!("     实际: {:?}\n", result);

    // 清理测试数据
    println!("\n=== 清理测试数据 ===\n");

    client
        .delete_file(
            "file1".to_string(),
            vec!["rust".to_string(), "distributed".to_string()],
        )
        .await?;

    client
        .delete_file(
            "file2".to_string(),
            vec!["rust".to_string(), "storage".to_string()],
        )
        .await?;

    client
        .delete_file(
            "file3".to_string(),
            vec!["python".to_string(), "storage".to_string()],
        )
        .await?;

    client
        .delete_file(
            "file4".to_string(),
            vec![
                "rust".to_string(),
                "storage".to_string(),
                "distributed".to_string(),
            ],
        )
        .await?;

    println!("\n=== 测试完成 ===");

    Ok(())
}
