use std::str::FromStr;

use eyre::{Result, eyre};
use regex::Regex;

use crate::token::{Tok, Token};

#[must_use]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Lexer {
    data: Vec<char>,
    token: String,
    tokens: Vec<Token>,
    idx: usize,
    string_quote: Option<char>, // What the quote character is, out of ', ", $
    spaces_in_quotes: bool,     // If quoted strings can contain spaces.
}

impl Lexer {
    pub fn new(data: &str) -> Result<Self> {
        let string_quote_rx = Regex::new(r"(?is)\(\s*string_quote\s+(.)\s*\)")?;
        let spaces_in_quotes_rx = Regex::new(r"(?is)\(\s*space_in_quoted_tokens\s+off\s*\)")?;

        let string_quote = if let Some(cap) = string_quote_rx.captures(data) {
            let quote = cap.get(1).ok_or_else(|| eyre!("expected quote chacater"))?;
            match quote.as_str() {
                "'" | "\"" | "$" => quote.as_str().chars().next(),
                x => return Err(eyre!("unknown string quote character {}", x)),
            }
        } else {
            None
        };
        // Default spaces_in_quotes to true if no directive found - most tools
        // do this even though it's technically against the spec.
        let spaces_in_quotes = !spaces_in_quotes_rx.is_match(data);

        // Remove these directives. At least the string quote needs to
        // be removed for proper lexing.
        let data = string_quote_rx.replace_all(data, "");
        let data = spaces_in_quotes_rx.replace_all(&data, "");

        Ok(Self {
            data: data.chars().collect(),
            token: String::new(),
            tokens: Vec::new(),
            idx: 0,
            string_quote,
            spaces_in_quotes,
        })
    }

    pub fn lex(mut self) -> Result<Vec<Token>> {
        while self.idx < self.data.len() {
            let c = self.next()?;
            if Some(c) == self.string_quote && self.spaces_in_quotes {
                // Grab quoted literal.
                while self.peek().ok_or_else(|| eyre!("unexpected EOF"))? != c {
                    let next = self.next()?;
                    self.token.push(next);
                }
                self.next()?; // Discard ending character
                self.push_literal();
            } else {
                // Ends current token:
                if c.is_whitespace() || c == '(' || c == ')' {
                    self.push_token();
                }
                if !c.is_whitespace() {
                    self.token.push(c);
                }
                // Is complete token:
                if c == '(' || c == ')' {
                    self.push_token();
                }
            }
        }
        self.push_token();
        Ok(self.tokens)
    }

    fn peek(&self) -> Option<char> {
        self.data.get(self.idx).copied()
    }

    fn next(&mut self) -> Result<char> {
        if self.idx < self.data.len() {
            self.idx += 1;
            Ok(self.data[self.idx - 1])
        } else {
            Err(eyre!("unexpected EOF"))
        }
    }

    fn push_token(&mut self) {
        if !self.token.is_empty() {
            let token_str = std::mem::take(&mut self.token);
            let token = Token {
                tok: Tok::from_str(&token_str.to_lowercase()).unwrap_or(Tok::Literal),
                s: token_str,
            };
            self.tokens.push(token);
        }
    }

    fn push_literal(&mut self) {
        let token = Token { tok: Tok::Literal, s: self.token.clone() };
        self.tokens.push(token);
        self.token.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_tokens() -> Result<()> {
        let data = "(pcb test)";
        let tokens = Lexer::new(data)?.lex()?;
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0].tok, Tok::Lparen);
        assert_eq!(tokens[1].tok, Tok::Pcb);
        assert_eq!(tokens[2].tok, Tok::Literal);
        assert_eq!(tokens[2].s, "test");
        assert_eq!(tokens[3].tok, Tok::Rparen);
        Ok(())
    }

    #[test]
    fn nested_expressions() -> Result<()> {
        let data = "(pcb (net test))";
        let tokens = Lexer::new(data)?.lex()?;
        assert_eq!(tokens.len(), 7);
        assert_eq!(tokens[0].tok, Tok::Lparen);
        assert_eq!(tokens[1].tok, Tok::Pcb);
        assert_eq!(tokens[2].tok, Tok::Lparen);
        assert_eq!(tokens[3].tok, Tok::Net);
        assert_eq!(tokens[4].tok, Tok::Literal);
        assert_eq!(tokens[4].s, "test");
        assert_eq!(tokens[5].tok, Tok::Rparen);
        assert_eq!(tokens[6].tok, Tok::Rparen);
        Ok(())
    }

    #[test]
    fn whitespace_handling() -> Result<()> {
        let data = "  (  pcb   test  )  ";
        let tokens = Lexer::new(data)?.lex()?;
        assert_eq!(tokens.len(), 4);
        assert_eq!(tokens[0].tok, Tok::Lparen);
        assert_eq!(tokens[1].tok, Tok::Pcb);
        assert_eq!(tokens[2].tok, Tok::Literal);
        assert_eq!(tokens[2].s, "test");
        assert_eq!(tokens[3].tok, Tok::Rparen);
        Ok(())
    }

    #[test]
    fn string_quote_double() -> Result<()> {
        let data = r#"(string_quote ") (net "test name")"#;
        let tokens = Lexer::new(data)?.lex()?;
        assert_eq!(tokens[1].tok, Tok::Net);
        assert_eq!(tokens[2].tok, Tok::Literal);
        assert_eq!(tokens[2].s, "test name");
        Ok(())
    }

    #[test]
    fn quoted_keyword_is_literal() -> Result<()> {
        let data = r#"(string_quote ") (net "pcb")"#;
        let tokens = Lexer::new(data)?.lex()?;
        assert_eq!(tokens[1].tok, Tok::Net);
        assert_eq!(tokens[2].tok, Tok::Literal);
        assert_eq!(tokens[2].s, "pcb");
        Ok(())
    }

    #[test]
    fn space_in_quoted_tokens_off() -> Result<()> {
        let data = r#"(string_quote ") (space_in_quoted_tokens off) (net "ab")"#;
        let tokens = Lexer::new(data)?.lex()?;
        assert_eq!(tokens[1].tok, Tok::Net);
        assert_eq!(tokens[2].tok, Tok::Literal);
        assert_eq!(tokens[2].s, "\"ab\"");
        Ok(())
    }

    #[test]
    fn space_in_quoted_tokens_off_spaces() -> Result<()> {
        let data = r#"(string_quote ") (space_in_quoted_tokens off) (net "a b")"#;
        let tokens = Lexer::new(data)?.lex()?;
        assert_eq!(tokens[1].tok, Tok::Net);
        assert_eq!(tokens[2].tok, Tok::Literal);
        assert_eq!(tokens[2].s, "\"a");
        assert_eq!(tokens[3].tok, Tok::Literal);
        assert_eq!(tokens[3].s, "b\"");
        Ok(())
    }

    #[test]
    fn string_quote_single() -> Result<()> {
        let data = "(string_quote ') (net 'test name')";
        let tokens = Lexer::new(data)?.lex()?;
        assert_eq!(tokens[1].tok, Tok::Net);
        assert_eq!(tokens[2].tok, Tok::Literal);
        assert_eq!(tokens[2].s, "test name");
        Ok(())
    }

    #[test]
    fn string_quote_dollar() -> Result<()> {
        let data = "(string_quote $) (net $test name$)";
        let tokens = Lexer::new(data)?.lex()?;
        assert_eq!(tokens[1].tok, Tok::Net);
        assert_eq!(tokens[2].tok, Tok::Literal);
        assert_eq!(tokens[2].s, "test name");
        Ok(())
    }

    #[test]
    fn unclosed_quoted_string_errors() -> Result<()> {
        let data = r#"(string_quote ") (pcb "unclosed string)"#;
        let lexer = Lexer::new(data)?;
        assert!(lexer.lex().is_err());
        Ok(())
    }

    #[test]
    fn empty_input() -> Result<()> {
        let data = "";
        let tokens = Lexer::new(data)?.lex()?;
        assert_eq!(tokens.len(), 0);
        Ok(())
    }

    #[test]
    fn only_whitespace() -> Result<()> {
        let data = "   \n\t  ";
        let tokens = Lexer::new(data)?.lex()?;
        assert_eq!(tokens.len(), 0);
        Ok(())
    }

    #[test]
    fn case_insensitive_keywords() -> Result<()> {
        let data = "(PCB NET VIA)";
        let tokens = Lexer::new(data)?.lex()?;
        assert_eq!(tokens[1].tok, Tok::Pcb);
        assert_eq!(tokens[2].tok, Tok::Net);
        assert_eq!(tokens[3].tok, Tok::Via);
        Ok(())
    }

    #[test]
    fn negative_numbers() -> Result<()> {
        let data = "(vertex -10.5 -20.3)";
        let tokens = Lexer::new(data)?.lex()?;
        assert_eq!(tokens[1].tok, Tok::Literal);
        assert_eq!(tokens[1].s, "vertex");
        assert_eq!(tokens[2].tok, Tok::Literal);
        assert_eq!(tokens[2].s, "-10.5");
        assert_eq!(tokens[3].tok, Tok::Literal);
        assert_eq!(tokens[3].s, "-20.3");
        Ok(())
    }

    #[test]
    fn identifiers_with_underscores() -> Result<()> {
        let data = "(net my_net_name_123)";
        let tokens = Lexer::new(data)?.lex()?;
        assert_eq!(tokens[2].tok, Tok::Literal);
        assert_eq!(tokens[2].s, "my_net_name_123");
        Ok(())
    }

    #[test]
    fn identifiers_with_dashes() -> Result<()> {
        let data = "(component R1-123-test)";
        let tokens = Lexer::new(data)?.lex()?;
        assert_eq!(tokens[2].tok, Tok::Literal);
        assert_eq!(tokens[2].s, "R1-123-test");
        Ok(())
    }

    #[test]
    fn real_pcb_snippet() -> Result<()> {
        let data = "(pcb test_board (resolution mm 1000))";
        let tokens = Lexer::new(data)?.lex()?;
        assert_eq!(tokens[0].tok, Tok::Lparen);
        assert_eq!(tokens[1].tok, Tok::Pcb);
        assert_eq!(tokens[2].tok, Tok::Literal);
        assert_eq!(tokens[2].s, "test_board");
        assert_eq!(tokens[3].tok, Tok::Lparen);
        assert_eq!(tokens[4].tok, Tok::Resolution);
        assert_eq!(tokens[5].tok, Tok::Mm);
        assert_eq!(tokens[6].tok, Tok::Literal);
        assert_eq!(tokens[6].s, "1000");
        Ok(())
    }

    #[test]
    fn multiline_input() -> Result<()> {
        let data = "(pcb test\n  (net mynet)\n  (via v1))";
        let tokens = Lexer::new(data)?.lex()?;
        assert_eq!(tokens[1].tok, Tok::Pcb);
        assert_eq!(tokens[2].tok, Tok::Literal);
        assert_eq!(tokens[2].s, "test");
        assert_eq!(tokens[4].tok, Tok::Net);
        assert_eq!(tokens[5].tok, Tok::Literal);
        assert_eq!(tokens[5].s, "mynet");
        assert_eq!(tokens[8].tok, Tok::Via);
        assert_eq!(tokens[9].tok, Tok::Literal);
        assert_eq!(tokens[9].s, "v1");
        Ok(())
    }

    #[test]
    fn quoted_empty_string() -> Result<()> {
        let data = r#"(string_quote ") (net "")"#;
        let tokens = Lexer::new(data)?.lex()?;
        let net_idx = tokens
            .iter()
            .position(|t| t.tok == Tok::Net)
            .ok_or_else(|| eyre!("expected net token"))?;
        assert_eq!(tokens[net_idx + 1].s, "");
        Ok(())
    }

    #[test]
    fn invalid_quote_character() {
        let data = r"(string_quote x) (pcb test)";
        assert!(Lexer::new(data).is_err());
    }
}
