use crate::lexer::{lookup_keyword::lookup_keyword, token::Token};

pub struct Lexer {
    input: Vec<char>,
    position: usize,
    current_char: Option<char>,
    line: usize,
    column: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        let chars: Vec<char> = input.chars().collect();

        Self {
            current_char: chars.get(0).copied(),
            input: chars,
            position: 0,
            line: 1,
            column: 1,
        }
    }

    fn advance(&mut self) {
        if let Some(ch) = self.current_char {
            if ch == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
        }

        self.position += 1;

        self.current_char = self.input.get(self.position).copied();
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.position + 1).copied()
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.current_char {
            if ch.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn skip_comment(&mut self) {
        if let Some(ch) = self.current_char {
            if ch == '-' && self.peek() == Some('-') {
                self.advance();
                self.advance();

                while let Some(c) = self.current_char {
                    if c == '\n' {
                        break;
                    }

                    self.advance();
                }
            }

            if ch == '/' && self.peek() == Some('*') {
                self.advance();
                self.advance();

                while let Some(c) = self.current_char {
                    if c == '*' && self.peek() == Some('/') {
                        self.advance();
                        self.advance();

                        break;
                    }
                    self.advance();
                }
            }
        }
    }

    fn read_identifier(&mut self) -> Token {
        let mut word = String::new();

        while let Some(ch) = self.current_char {
            if ch.is_alphanumeric() || ch == '_' {
                word.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        lookup_keyword(&word)
    }

    fn read_number(&mut self) -> Token {
        let mut number = String::new();

        let mut has_dot = false;

        while let Some(ch) = self.current_char {
            if ch.is_ascii_digit() {
                number.push(ch);
                self.advance();
            } else if ch == '.' && !has_dot {
                has_dot = true;
                number.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        if has_dot {
            Token::FloatLit(number.parse::<f64>().unwrap())
        } else {
            Token::IntLit(number.parse::<i64>().unwrap())
        }
    }

    fn read_string(&mut self) -> Token {
        let mut value = String::new();

        self.advance();

        while let Some(ch) = self.current_char {
            if ch == '\'' && self.peek() == Some('\'') {
                value.push('\'');
                self.advance();
                self.advance();
            } else if ch == '\'' {
                break;
            } else {
                value.push(ch);
                self.advance();
            }
        }
        self.advance();

        Token::StringLit(value)
    }

    fn read_quoted_identifier(&mut self) -> Token {
        let mut value = String::new();

        self.advance();

        while let Some(ch) = self.current_char {
            if ch == '\"' && self.peek() == Some('\"') {
                value.push('\'');
                self.advance();
                self.advance();
            } else if ch == '\"' {
                break;
            } else {
                value.push(ch);
                self.advance();
            }
        }
        self.advance();

        Token::QuotedIdent(value)
    }

    pub fn next_token(&mut self) -> Token {
        loop {
            self.skip_whitespace();

            let is_comment = (self.current_char == Some('-') && self.peek() == Some('-'))
                || (self.current_char == Some('/') && self.peek() == Some('*'));

            if is_comment {
                self.skip_comment();
            } else {
                break;
            }
        }

        if let Some(ch) = self.current_char {
            match ch {
                // Identifier or Keywords
                c if c.is_alphabetic() || c == '_' => {
                    return self.read_identifier();
                }
                // Numbers
                c if c.is_ascii_digit() => {
                    return self.read_number();
                }
                // Strings
                '\'' => {
                    return self.read_string();
                }
                '\"' => {
                    return self.read_quoted_identifier();
                }
                // Operators
                '=' => {
                    self.advance();
                    return Token::Eq;
                }
                '!' => {
                    if self.peek() == Some('=') {
                        self.advance();
                        self.advance();
                        return Token::Ne;
                    } else {
                        self.advance();
                        return Token::Illegal('!');
                    }
                }
                '<' => {
                    if self.peek() == Some('=') {
                        self.advance();
                        self.advance();
                        return Token::Le;
                    } else if self.peek() == Some('>') {
                        self.advance();
                        self.advance();
                        return Token::Ne;
                    }

                    self.advance();
                    return Token::Lt;
                }

                '>' => {
                    if self.peek() == Some('=') {
                        self.advance();
                        self.advance();
                        return Token::Ge;
                    }

                    self.advance();
                    return Token::Gt;
                }

                '+' => {
                    self.advance();
                    return Token::Plus;
                }

                '-' => {
                    if self.peek() == Some('>') {
                        self.advance();
                        if self.peek() == Some('>') {
                            self.advance();
                            self.advance();
                            return Token::DoubleArrow;
                        } else {
                            self.advance();
                            return Token::Arrow;
                        }
                    } else {
                        self.advance();
                        return Token::Minus;
                    }
                }

                '*' => {
                    self.advance();
                    return Token::Star;
                }

                '/' => {
                    self.advance();
                    return Token::Slash;
                }

                '%' => {
                    self.advance();
                    return Token::Percent;
                }

                // punctuation
                '(' => {
                    self.advance();
                    return Token::LParen;
                }

                ')' => {
                    self.advance();
                    return Token::RParen;
                }

                ',' => {
                    self.advance();
                    return Token::Comma;
                }

                ';' => {
                    self.advance();
                    return Token::Semicolon;
                }

                '.' => {
                    self.advance();
                    return Token::Dot;
                }

                '|' => {
                    if self.peek() == Some('|') {
                        self.advance();
                        self.advance();
                        return Token::Concat;
                    } else {
                        self.advance();
                        return Token::Illegal('|');
                    }
                }

                ':' => {
                    if self.peek() == Some(':') {
                        self.advance();
                        self.advance();
                        return Token::DoubleColon;
                    } else {
                        self.advance();
                        return Token::Illegal(':');
                    }
                }

                // unknown character
                _ => {
                    self.advance();
                    return Token::Illegal(ch);
                }
            }
        }

        Token::Eof
    }
}
