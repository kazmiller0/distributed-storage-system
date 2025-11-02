use common::rpc::{
    manager_service_client::ManagerServiceClient, AddRequest, DeleteRequest, QueryRequest,
    UpdateRequest,
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
            println!("Put file succeeded: {}", resp.message);
        } else {
            println!("Put file failed: {}", resp.message);
        }

        Ok(())
    }

    // Query by keyword
    pub async fn query_by_keyword(
        &self,
        keyword: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut client = ManagerServiceClient::connect(self.manager_addr.clone()).await?;

        let request = QueryRequest {
            query_type: Some(common::rpc::query_request::QueryType::Keyword(keyword)),
        };

        let response = client.query(request).await?;
        let resp = response.into_inner();

        if resp.verified {
            println!("Query succeeded, found {} files:", resp.fids.len());
            for fid in resp.fids {
                println!("  - {}", fid);
            }
        } else {
            println!("Query verification failed!");
        }

        Ok(())
    }

    // Query by boolean function (not yet implemented)
    pub async fn query_by_func(
        &self,
        boolean_func: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut client = ManagerServiceClient::connect(self.manager_addr.clone()).await?;

        let request = QueryRequest {
            query_type: Some(common::rpc::query_request::QueryType::BooleanFunction(
                boolean_func,
            )),
        };

        let response = client.query(request).await?;
        let resp = response.into_inner();

        if resp.verified {
            println!("Query succeeded, found {} files:", resp.fids.len());
            for fid in resp.fids {
                println!("  - {}", fid);
            }
        } else {
            println!("Query verification failed!");
        }

        Ok(())
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
            println!("Delete file succeeded: {}", resp.message);
        } else {
            println!("Delete file failed: {}", resp.message);
        }

        Ok(())
    }

    // Update file: change (fid, old_keywords) to (fid, new_keywords)
    pub async fn update_file(
        &self,
        fid: String,
        old_keywords: Vec<String>,
        new_keywords: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut client = ManagerServiceClient::connect(self.manager_addr.clone()).await?;

        let request = UpdateRequest {
            fid,
            old_keywords,
            new_keywords,
        };

        let response = client.update(request).await?;
        let resp = response.into_inner();

        if resp.success {
            println!("Update file succeeded: {}", resp.message);
        } else {
            println!("Update file failed: {}", resp.message);
        }

        Ok(())
    }
}

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
