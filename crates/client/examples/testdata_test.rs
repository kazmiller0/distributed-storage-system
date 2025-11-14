use common::rpc::{
    manager_service_client::ManagerServiceClient, AddRequest, DeleteRequest, QueryRequest,
    UpdateRequest,
};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Instant;

// Client structure
pub struct Client {
    manager_addr: String,
}

impl Client {
    pub fn new(manager_addr: String) -> Self {
        Client { manager_addr }
    }

    // Add file with keywords
    pub async fn add_file(
        &self,
        fid: String,
        keywords: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut client = ManagerServiceClient::connect(self.manager_addr.clone()).await?;
        let request = AddRequest { fid, keywords };
        let response = client.add(request).await?;
        let resp = response.into_inner();

        if !resp.success {
            return Err(format!("Add failed: {}", resp.message).into());
        }
        Ok(())
    }

    // Query by keyword
    pub async fn query_keyword(
        &self,
        keyword: String,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut client = ManagerServiceClient::connect(self.manager_addr.clone()).await?;
        let request = QueryRequest {
            query_type: Some(common::rpc::query_request::QueryType::Keyword(keyword)),
        };
        let response = client.query(request).await?;
        let resp = response.into_inner();

        if !resp.verified {
            return Err("Query verification failed!".into());
        }
        Ok(resp.fids)
    }

    // Query by boolean function
    pub async fn query_boolean(
        &self,
        boolean_func: String,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut client = ManagerServiceClient::connect(self.manager_addr.clone()).await?;
        let request = QueryRequest {
            query_type: Some(common::rpc::query_request::QueryType::BooleanFunction(
                boolean_func,
            )),
        };
        let response = client.query(request).await?;
        let resp = response.into_inner();

        if !resp.verified {
            return Err("Query verification failed!".into());
        }
        Ok(resp.fids)
    }

    // Delete file
    pub async fn delete_file(
        &self,
        fid: String,
        keywords: Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut client = ManagerServiceClient::connect(self.manager_addr.clone()).await?;
        let request = DeleteRequest { fid, keywords };
        let response = client.delete(request).await?;
        let resp = response.into_inner();

        if !resp.success {
            return Err(format!("Delete failed: {}", resp.message).into());
        }
        Ok(())
    }

    // Update file
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

        if !resp.success {
            return Err(format!("Update failed: {}", resp.message).into());
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager_addr = "http://[::1]:50051".to_string();
    let client = Client::new(manager_addr);

    println!("测试数据文件导入测试");
    println!("===========================================================\n");

    // Read testdata file
    let file = File::open("data/testdata")?;
    let reader = BufReader::new(file);
    let mut data_entries = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let parts: Vec<String> = line.split(',').map(|s| s.trim().to_string()).collect();
        if parts.len() >= 2 {
            let fid = parts[0].clone();
            let keywords = parts[1..].to_vec();
            data_entries.push((fid, keywords));
        }
    }

    println!("加载了 {} 条数据记录\n", data_entries.len());

    // Test 1: Batch Add
    println!("测试 1: 批量添加数据");
    println!("-----------------------------------------------------------");
    let start = Instant::now();
    let mut success_count = 0;
    let mut fail_count = 0;

    for (i, (fid, keywords)) in data_entries.iter().enumerate() {
        match client.add_file(fid.clone(), keywords.clone()).await {
            Ok(_) => {
                success_count += 1;
                if (i + 1) % 20 == 0 {
                    println!("  进度: {}/{} 条记录已添加", i + 1, data_entries.len());
                }
            }
            Err(e) => {
                fail_count += 1;
                println!("  添加 {} 失败: {}", fid, e);
            }
        }
    }

    let duration = start.elapsed();
    println!("\n  添加完成: {} 成功, {} 失败", success_count, fail_count);
    println!("  耗时: {:.2?}", duration);
    println!(
        "  平均速度: {:.2} 条/秒\n",
        success_count as f64 / duration.as_secs_f64()
    );

    // Test 2: Single keyword queries
    println!("测试 2: 单关键词查询");
    println!("-----------------------------------------------------------");

    let test_keywords = vec!["animal", "red", "wooden", "black", "white"];

    for keyword in &test_keywords {
        let start = Instant::now();
        match client.query_keyword(keyword.to_string()).await {
            Ok(results) => {
                let duration = start.elapsed();
                println!(
                    "  '{}': 找到 {} 个文件 (耗时: {:.2?})",
                    keyword,
                    results.len(),
                    duration
                );
                println!("    结果: {:?}", results);
            }
            Err(e) => println!("  查询 '{}' 失败: {}", keyword, e),
        }
    }
    println!();

    // Test 3: Boolean queries
    println!("测试 3: 布尔查询");
    println!("-----------------------------------------------------------");

    let boolean_tests = vec![
        ("animal AND red", "红色动物"),
        ("flower AND pink", "粉色花朵"),
        ("clothes AND blue", "蓝色衣物"),
        ("food OR beverage", "食物或饮料"),
        ("wooden AND furniture", "木制家具"),
        ("electronics AND black", "黑色电子产品"),
        ("(animal OR flower) AND red", "红色的动物或花朵"),
        ("sport AND (white OR orange)", "白色或橙色的运动用品"),
    ];

    for (query, description) in &boolean_tests {
        let start = Instant::now();
        match client.query_boolean(query.to_string()).await {
            Ok(results) => {
                let duration = start.elapsed();
                println!(
                    "  {} [{}]: 找到 {} 个文件 (耗时: {:.2?})",
                    description,
                    query,
                    results.len(),
                    duration
                );
                println!("    结果: {:?}", results);
            }
            Err(e) => println!("  查询 '{}' 失败: {}", query, e),
        }
    }
    println!();

    // Test 4: Update operation
    println!("测试 4: 更新操作");
    println!("-----------------------------------------------------------");

    // Update first 5 entries: change one keyword
    let update_count = 5;
    let start = Instant::now();
    let mut updated = 0;

    println!("  更新详情:");
    for (fid, keywords) in data_entries.iter().take(update_count) {
        if keywords.len() >= 2 {
            // Remove first keyword, add a new one
            let old_kw = vec![keywords[0].clone()];
            let new_kw = vec!["updated".to_string()];
            
            println!("    {} → 旧关键词: {:?}, 新关键词: {:?}", 
                     fid, old_kw, new_kw);
            
            match client
                .update_file(fid.clone(), old_kw.clone(), new_kw.clone())
                .await
            {
                Ok(_) => {
                    updated += 1;
                    println!("      ✓ 更新成功");
                }
                Err(e) => println!("      ✗ 更新失败: {}", e),
            }
        }
    }

    let duration = start.elapsed();
    println!("\n  更新完成: {} 条记录 (耗时: {:.2?})\n", updated, duration);

    // Verify update - check old keyword removed
    println!("  验证更新结果:");
    let first_old_keyword = &data_entries[0].1[0];
    match client.query_keyword(first_old_keyword.clone()).await {
        Ok(results) => {
            let first_fid = &data_entries[0].0;
            if results.contains(first_fid) {
                println!("    ✗ 旧关键词 '{}' 仍然关联到 {}", first_old_keyword, first_fid);
            } else {
                println!("    ✓ 旧关键词 '{}' 已从 {} 移除", first_old_keyword, first_fid);
            }
        }
        Err(e) => println!("    验证查询失败: {}", e),
    }
    
    // Verify new keyword added
    match client.query_keyword("updated".to_string()).await {
        Ok(results) => {
            println!("    ✓ 新关键词 'updated': 找到 {} 个文件", results.len());
            println!("      结果: {:?}", results);
        }
        Err(e) => println!("    验证查询失败: {}", e),
    }
    println!();
    match client.query_keyword("updated".to_string()).await {
        Ok(results) => {
            println!("    查询 'updated': 找到 {} 个文件", results.len());
            if results.len() <= 10 {
                println!("    结果: {:?}", results);
            }
        }
        Err(e) => println!("    验证查询失败: {}", e),
    }
    println!();

    // Test 5: Delete some entries
    println!("测试 5: 删除部分数据");
    println!("-----------------------------------------------------------");

    let delete_count = 10;
    let start = Instant::now();
    let mut deleted = 0;

    println!("  删除详情:");
    for (fid, keywords) in data_entries.iter().take(delete_count) {
        println!("    {} → 关键词: {:?}", fid, keywords);
        match client.delete_file(fid.clone(), keywords.clone()).await {
            Ok(_) => {
                deleted += 1;
                println!("      ✓ 删除成功");
            }
            Err(e) => println!("      ✗ 删除失败: {}", e),
        }
    }

    let duration = start.elapsed();
    println!("\n  删除完成: {} 条记录 (耗时: {:.2?})\n", deleted, duration);

    // Verify deletion - check multiple deleted files
    println!("  验证删除结果:");
    
    // Check first deleted file
    let first_id = &data_entries[0].0;
    let first_keyword = &data_entries[0].1[0];
    match client.query_keyword(first_keyword.clone()).await {
        Ok(results) => {
            if results.contains(first_id) {
                println!("    ✗ {} 仍然存在于关键词 '{}' 的查询结果中", first_id, first_keyword);
            } else {
                println!("    ✓ {} 已从关键词 '{}' 中删除", first_id, first_keyword);
            }
        }
        Err(e) => println!("    验证查询失败: {}", e),
    }
    
    // Check last deleted file
    let last_id = &data_entries[delete_count - 1].0;
    let last_keyword = &data_entries[delete_count - 1].1[0];
    match client.query_keyword(last_keyword.clone()).await {
        Ok(results) => {
            if results.contains(last_id) {
                println!("    ✗ {} 仍然存在于关键词 '{}' 的查询结果中", last_id, last_keyword);
            } else {
                println!("    ✓ {} 已从关键词 '{}' 中删除", last_id, last_keyword);
            }
        }
        Err(e) => println!("    验证查询失败: {}", e),
    }
    
    // Check a category that had deleted items
    println!("\n  删除前后对比:");
    match client.query_keyword("animal".to_string()).await {
        Ok(results) => {
            println!("    关键词 'animal': 剩余 {} 个文件", results.len());
            println!("    结果: {:?}", results);
            println!("    (原有 8 个,删除了 {} 中的动物相关)", delete_count);
        }
        Err(e) => println!("    对比查询失败: {}", e),
    }
    println!();

    println!("===========================================================");
    println!("测试完成");
    println!("===========================================================");
    println!("\n测试总结:");
    println!("  - 数据条目: {} 条", data_entries.len());
    println!("  - 添加成功: {} 条", success_count);
    println!("  - 更新成功: {} 条", updated);
    println!("  - 删除成功: {} 条", deleted);
    println!("  - 最终数据量: {} 条", success_count - deleted);

    Ok(())
}
