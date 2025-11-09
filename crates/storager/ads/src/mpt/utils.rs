/// 键值对结构，对应 Go 中的 util.KVPair
#[derive(Debug, Clone)]
pub struct KVPair {
    key: String,
    value: String,
}

impl KVPair {
    pub fn new(key: String, value: String) -> Self {
        Self { key, value }
    }

    pub fn get_key(&self) -> &str {
        &self.key
    }

    pub fn get_value(&self) -> &str {
        &self.value
    }

    /// 添加值到现有值中（用于辅助索引）
    pub fn add_value(&mut self, new_value: &str) -> bool {
        if self.value.is_empty() {
            self.value = new_value.to_string();
            true
        } else if !self.value.contains(new_value) {
            self.value = format!("{},{}", self.value, new_value);
            true
        } else {
            false
        }
    }

    /// 从现有值中删除值（用于辅助索引）
    pub fn del_value(&mut self, del_value: &str) -> bool {
        if self.value.contains(del_value) {
            let values: Vec<&str> = self.value.split(',').filter(|&v| v != del_value).collect();
            self.value = values.join(",");
            true
        } else {
            false
        }
    }
}

/// 字节转十六进制索引 (支持任意字节值，不只是hex字符)
pub fn byte_to_hex_index(byte: u8) -> usize {
    (byte & 0x0F) as usize // 取低4位作为索引，范围0-15
}

/// 字符转十六进制索引（用于字符串key）
pub fn char_to_hex_index(byte: u8) -> Option<usize> {
    match byte {
        b'0'..=b'9' => Some((byte - b'0') as usize),
        b'a'..=b'f' => Some((byte - b'a' + 10) as usize),
        b'A'..=b'F' => Some((byte - b'A' + 10) as usize),
        _ => None,
    }
}

/// 将键转换为适合 Trie 的路径（每个字节拆分为两个半字节）
pub fn key_to_hex_path(key: &str) -> Vec<u8> {
    let mut path = Vec::new();
    for byte in key.as_bytes() {
        path.push((byte >> 4) & 0x0F); // 高4位
        path.push(byte & 0x0F); // 低4位
    }
    path
}

/// 从十六进制路径重构键的字节表示
pub fn hex_path_to_bytes(path: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for chunk in path.chunks(2) {
        if chunk.len() == 2 {
            bytes.push((chunk[0] << 4) | chunk[1]);
        } else if chunk.len() == 1 {
            bytes.push(chunk[0] << 4); // 奇数长度，高4位
        }
    }
    bytes
}

/// 从十六进制路径重构原始字符串键
pub fn hex_path_to_key(path: &[u8]) -> String {
    let bytes = hex_path_to_bytes(path);
    String::from_utf8_lossy(&bytes).to_string()
}

/// 计算两个字符串的公共前缀
pub fn common_prefix(str1: &str, str2: &str) -> String {
    let bytes1 = str1.as_bytes();
    let bytes2 = str2.as_bytes();
    let min_len = bytes1.len().min(bytes2.len());

    let mut common_len = 0;
    for i in 0..min_len {
        if bytes1[i] == bytes2[i] {
            common_len += 1;
        } else {
            break;
        }
    }

    str1[..common_len].to_string()
}

/// 计算两个字节向量的公共前缀长度
pub fn common_prefix_len(vec1: &[u8], vec2: &[u8]) -> usize {
    let min_len = vec1.len().min(vec2.len());
    let mut common_len = 0;

    for i in 0..min_len {
        if vec1[i] == vec2[i] {
            common_len += 1;
        } else {
            break;
        }
    }

    common_len
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kvpair() {
        let mut kv = KVPair::new("key1".to_string(), "value1".to_string());
        assert_eq!(kv.get_key(), "key1");
        assert_eq!(kv.get_value(), "value1");

        assert!(kv.add_value("value2"));
        assert_eq!(kv.get_value(), "value1,value2");

        assert!(kv.del_value("value1"));
        assert_eq!(kv.get_value(), "value2");
    }

    #[test]
    fn test_byte_to_hex_index() {
        // 新的 byte_to_hex_index 函数测试（任意字节）
        assert_eq!(byte_to_hex_index(0), 0);
        assert_eq!(byte_to_hex_index(15), 15);
        assert_eq!(byte_to_hex_index(16), 0); // 0x10 & 0x0F = 0
        assert_eq!(byte_to_hex_index(255), 15); // 0xFF & 0x0F = 15

        // 测试字符转hex索引函数
        assert_eq!(char_to_hex_index(b'0'), Some(0));
        assert_eq!(char_to_hex_index(b'9'), Some(9));
        assert_eq!(char_to_hex_index(b'a'), Some(10));
        assert_eq!(char_to_hex_index(b'f'), Some(15));
        assert_eq!(char_to_hex_index(b'A'), Some(10));
        assert_eq!(char_to_hex_index(b'F'), Some(15));
        assert_eq!(char_to_hex_index(b'g'), None);
    }

    #[test]
    fn test_key_conversion() {
        // 测试键到十六进制路径的转换
        let path = key_to_hex_path("ab");
        assert_eq!(path, vec![6, 1, 6, 2]); // 'a'=0x61 -> [6,1], 'b'=0x62 -> [6,2]

        // 测试路径到键的转换
        let key = hex_path_to_key(&[6, 1, 6, 2]);
        assert_eq!(key, "ab");

        // 测试路径到字节的转换
        let bytes = hex_path_to_bytes(&[6, 1, 6, 2]);
        assert_eq!(bytes, vec![0x61, 0x62]); // 'a', 'b'
    }

    #[test]
    fn test_common_prefix() {
        assert_eq!(common_prefix("hello", "help"), "hel");
        assert_eq!(common_prefix("abc", "def"), "");
        assert_eq!(common_prefix("same", "same"), "same");
        assert_eq!(common_prefix("", "anything"), "");
    }

    #[test]
    fn test_common_prefix_len() {
        let vec1 = vec![1, 2, 3, 4, 5];
        let vec2 = vec![1, 2, 3, 9, 10];
        assert_eq!(common_prefix_len(&vec1, &vec2), 3);

        let vec3 = vec![1, 2];
        let vec4 = vec![3, 4];
        assert_eq!(common_prefix_len(&vec3, &vec4), 0);

        let vec5 = vec![1, 2, 3];
        let vec6 = vec![1, 2, 3, 4];
        assert_eq!(common_prefix_len(&vec5, &vec6), 3);
    }
}
