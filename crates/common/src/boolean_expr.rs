//! 布尔表达式解析器
//!
//! 支持 AND, OR, NOT 运算符和括号
//!
//! # 语法
//!
//! ```text
//! Expression := Term (OR Term)*
//! Term       := Factor (AND Factor)*
//! Factor     := NOT Factor | Keyword | '(' Expression ')'
//! Keyword    := 标识符
//! ```
//!
//! # 示例
//!
//! ```
//! use common::boolean_expr::{BooleanExpr, parse_boolean_expr};
//!
//! // 简单查询
//! let expr = parse_boolean_expr("rust").unwrap();
//!
//! // AND 查询
//! let expr = parse_boolean_expr("rust AND storage").unwrap();
//!
//! // OR 查询
//! let expr = parse_boolean_expr("rust OR python").unwrap();
//!
//! // 复杂查询
//! let expr = parse_boolean_expr("(rust OR python) AND (storage OR database)").unwrap();
//!
//! // NOT 查询
//! let expr = parse_boolean_expr("rust AND NOT python").unwrap();
//! ```

use std::collections::HashSet;

/// 布尔表达式抽象语法树
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BooleanExpr {
    /// 单个关键词
    Keyword(String),
    /// AND 运算: 交集
    And(Box<BooleanExpr>, Box<BooleanExpr>),
    /// OR 运算: 并集
    Or(Box<BooleanExpr>, Box<BooleanExpr>),
    /// NOT 运算: 补集
    Not(Box<BooleanExpr>),
}

impl BooleanExpr {
    /// 获取表达式中所有的关键词
    pub fn get_keywords(&self) -> HashSet<String> {
        match self {
            BooleanExpr::Keyword(kw) => {
                let mut set = HashSet::new();
                set.insert(kw.clone());
                set
            }
            BooleanExpr::And(left, right) | BooleanExpr::Or(left, right) => {
                let mut set = left.get_keywords();
                set.extend(right.get_keywords());
                set
            }
            BooleanExpr::Not(expr) => expr.get_keywords(),
        }
    }

    /// 对结果集合求值
    ///
    /// # 参数
    ///
    /// * `keyword_results` - 每个关键词对应的文件ID集合
    ///
    /// # 返回
    ///
    /// 布尔表达式求值后的文件ID集合
    pub fn evaluate(
        &self,
        keyword_results: &std::collections::HashMap<String, HashSet<String>>,
    ) -> HashSet<String> {
        match self {
            BooleanExpr::Keyword(kw) => keyword_results.get(kw).cloned().unwrap_or_default(),
            BooleanExpr::And(left, right) => {
                let left_result = left.evaluate(keyword_results);
                let right_result = right.evaluate(keyword_results);
                left_result.intersection(&right_result).cloned().collect()
            }
            BooleanExpr::Or(left, right) => {
                let left_result = left.evaluate(keyword_results);
                let right_result = right.evaluate(keyword_results);
                left_result.union(&right_result).cloned().collect()
            }
            BooleanExpr::Not(expr) => {
                // For NOT operation, return empty set
                // (NOT operation is tricky without a universal set)
                let _result = expr.evaluate(keyword_results);
                HashSet::new()
            }
        }
    }

    /// 转换为字符串表示
    pub fn to_string(&self) -> String {
        match self {
            BooleanExpr::Keyword(kw) => kw.clone(),
            BooleanExpr::And(left, right) => {
                format!("({} AND {})", left.to_string(), right.to_string())
            }
            BooleanExpr::Or(left, right) => {
                format!("({} OR {})", left.to_string(), right.to_string())
            }
            BooleanExpr::Not(expr) => {
                format!("(NOT {})", expr.to_string())
            }
        }
    }
}

/// 词法分析器的 Token
#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Keyword(String),
    And,
    Or,
    Not,
    LeftParen,
    RightParen,
    Eof,
}

/// 词法分析器
struct Lexer {
    input: Vec<char>,
    pos: usize,
}

impl Lexer {
    fn new(input: &str) -> Self {
        Lexer {
            input: input.chars().collect(),
            pos: 0,
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() && self.input[self.pos].is_whitespace() {
            self.pos += 1;
        }
    }

    fn peek_char(&self) -> Option<char> {
        if self.pos < self.input.len() {
            Some(self.input[self.pos])
        } else {
            None
        }
    }

    fn read_char(&mut self) -> Option<char> {
        if self.pos < self.input.len() {
            let ch = self.input[self.pos];
            self.pos += 1;
            Some(ch)
        } else {
            None
        }
    }

    fn read_identifier(&mut self) -> String {
        let start = self.pos;
        while let Some(ch) = self.peek_char() {
            if ch.is_alphanumeric() || ch == '_' || ch == '-' {
                self.read_char();
            } else {
                break;
            }
        }
        self.input[start..self.pos].iter().collect()
    }

    fn next_token(&mut self) -> Token {
        self.skip_whitespace();

        match self.peek_char() {
            None => Token::Eof,
            Some('(') => {
                self.read_char();
                Token::LeftParen
            }
            Some(')') => {
                self.read_char();
                Token::RightParen
            }
            Some(ch) if ch.is_alphabetic() || ch == '_' => {
                let ident = self.read_identifier();
                match ident.to_uppercase().as_str() {
                    "AND" => Token::And,
                    "OR" => Token::Or,
                    "NOT" => Token::Not,
                    _ => Token::Keyword(ident),
                }
            }
            Some(_) => {
                // 处理其他字符作为关键词的一部分
                let ident = self.read_identifier();
                if ident.is_empty() {
                    self.read_char(); // 跳过无效字符
                    self.next_token()
                } else {
                    Token::Keyword(ident)
                }
            }
        }
    }
}

/// 递归下降解析器
struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(input: &str) -> Self {
        let mut lexer = Lexer::new(input);
        let mut tokens = Vec::new();

        loop {
            let token = lexer.next_token();
            if token == Token::Eof {
                tokens.push(token);
                break;
            }
            tokens.push(token);
        }

        Parser { tokens, pos: 0 }
    }

    fn current_token(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn advance(&mut self) {
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
    }

    fn expect(&mut self, expected: Token) -> Result<(), String> {
        if self.current_token() == &expected {
            self.advance();
            Ok(())
        } else {
            Err(format!(
                "Expected {:?}, found {:?}",
                expected,
                self.current_token()
            ))
        }
    }

    /// Expression := Term (OR Term)*
    fn parse_expression(&mut self) -> Result<BooleanExpr, String> {
        let mut left = self.parse_term()?;

        while self.current_token() == &Token::Or {
            self.advance(); // consume OR
            let right = self.parse_term()?;
            left = BooleanExpr::Or(Box::new(left), Box::new(right));
        }

        Ok(left)
    }

    /// Term := Factor (AND Factor)*
    fn parse_term(&mut self) -> Result<BooleanExpr, String> {
        let mut left = self.parse_factor()?;

        while self.current_token() == &Token::And {
            self.advance(); // consume AND
            let right = self.parse_factor()?;
            left = BooleanExpr::And(Box::new(left), Box::new(right));
        }

        Ok(left)
    }

    /// Factor := NOT Factor | Keyword | '(' Expression ')'
    fn parse_factor(&mut self) -> Result<BooleanExpr, String> {
        match self.current_token() {
            Token::Not => {
                self.advance(); // consume NOT
                let expr = self.parse_factor()?;
                Ok(BooleanExpr::Not(Box::new(expr)))
            }
            Token::Keyword(kw) => {
                let keyword = kw.clone();
                self.advance();
                Ok(BooleanExpr::Keyword(keyword))
            }
            Token::LeftParen => {
                self.advance(); // consume (
                let expr = self.parse_expression()?;
                self.expect(Token::RightParen)?;
                Ok(expr)
            }
            token => Err(format!("Unexpected token: {:?}", token)),
        }
    }

    fn parse(&mut self) -> Result<BooleanExpr, String> {
        let expr = self.parse_expression()?;
        if self.current_token() != &Token::Eof {
            return Err(format!(
                "Unexpected token after expression: {:?}",
                self.current_token()
            ));
        }
        Ok(expr)
    }
}

/// 解析布尔表达式
///
/// # 示例
///
/// ```
/// use common::boolean_expr::parse_boolean_expr;
///
/// let expr = parse_boolean_expr("rust AND storage").unwrap();
/// println!("{}", expr.to_string());
/// ```
pub fn parse_boolean_expr(input: &str) -> Result<BooleanExpr, String> {
    let mut parser = Parser::new(input);
    parser.parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_keyword() {
        let expr = parse_boolean_expr("rust").unwrap();
        assert_eq!(expr, BooleanExpr::Keyword("rust".to_string()));
    }

    #[test]
    fn test_parse_and() {
        let expr = parse_boolean_expr("rust AND storage").unwrap();
        assert!(matches!(expr, BooleanExpr::And(_, _)));
    }

    #[test]
    fn test_parse_or() {
        let expr = parse_boolean_expr("rust OR python").unwrap();
        assert!(matches!(expr, BooleanExpr::Or(_, _)));
    }

    #[test]
    fn test_parse_not() {
        let expr = parse_boolean_expr("NOT rust").unwrap();
        assert!(matches!(expr, BooleanExpr::Not(_)));
    }

    #[test]
    fn test_parse_complex() {
        let expr = parse_boolean_expr("(rust OR python) AND storage").unwrap();
        assert!(matches!(expr, BooleanExpr::And(_, _)));
    }

    #[test]
    fn test_parse_with_parens() {
        let expr = parse_boolean_expr("(rust AND storage) OR (python AND database)").unwrap();
        assert!(matches!(expr, BooleanExpr::Or(_, _)));
    }

    #[test]
    fn test_get_keywords() {
        let expr = parse_boolean_expr("(rust OR python) AND storage").unwrap();
        let keywords = expr.get_keywords();
        assert_eq!(keywords.len(), 3);
        assert!(keywords.contains("rust"));
        assert!(keywords.contains("python"));
        assert!(keywords.contains("storage"));
    }

    #[test]
    fn test_evaluate_and() {
        let expr = parse_boolean_expr("rust AND storage").unwrap();
        let mut results = std::collections::HashMap::new();

        let mut rust_files = HashSet::new();
        rust_files.insert("file1".to_string());
        rust_files.insert("file2".to_string());

        let mut storage_files = HashSet::new();
        storage_files.insert("file2".to_string());
        storage_files.insert("file3".to_string());

        results.insert("rust".to_string(), rust_files);
        results.insert("storage".to_string(), storage_files);

        let result = expr.evaluate(&results);
        assert_eq!(result.len(), 1);
        assert!(result.contains("file2"));
    }

    #[test]
    fn test_evaluate_or() {
        let expr = parse_boolean_expr("rust OR python").unwrap();
        let mut results = std::collections::HashMap::new();

        let mut rust_files = HashSet::new();
        rust_files.insert("file1".to_string());

        let mut python_files = HashSet::new();
        python_files.insert("file2".to_string());

        results.insert("rust".to_string(), rust_files);
        results.insert("python".to_string(), python_files);

        let result = expr.evaluate(&results);
        assert_eq!(result.len(), 2);
        assert!(result.contains("file1"));
        assert!(result.contains("file2"));
    }

    #[test]
    fn test_case_insensitive_operators() {
        assert!(parse_boolean_expr("rust and storage").is_ok());
        assert!(parse_boolean_expr("rust or python").is_ok());
        assert!(parse_boolean_expr("not rust").is_ok());
    }
}
