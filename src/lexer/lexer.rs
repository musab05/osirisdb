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

    pub fn current_position(&self) -> (usize, usize) {
        (self.line, self.column)
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

    fn skip_comment(&mut self) -> Result<(), Token> {
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
            } else if ch == '/' && self.peek() == Some('*') {
                let start_line = self.line;
                let start_col = self.column;
                let mut depth = 1;

                self.advance();
                self.advance();

                while let Some(c) = self.current_char {
                    if c == '/' && self.peek() == Some('*') {
                        depth += 1;
                        self.advance();
                        self.advance();
                    } else if c == '*' && self.peek() == Some('/') {
                        depth -= 1;
                        self.advance();
                        self.advance();

                        if depth == 0 {
                            break;
                        }
                    } else {
                        self.advance();
                    }
                }

                if depth > 0 {
                    return Err(Token::UnterminatedComment(start_line, start_col));
                }
            }
        }
        Ok(())
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
        let mut has_exp = false;

        while let Some(ch) = self.current_char {
            if ch.is_ascii_digit() {
                number.push(ch);
                self.advance();
            } else if ch == '.' && !has_dot {
                if self.peek() == Some('.') {
                    break;
                }
                has_dot = true;
                number.push(ch);
                self.advance();
            } else if (ch == 'e' || ch == 'E') && !has_exp {
                has_exp = true;
                number.push(ch);
                self.advance();
                if self.current_char == Some('+') || self.current_char == Some('-') {
                    number.push(self.current_char.unwrap());
                    self.advance();
                }
            } else {
                break;
            }
        }

        if has_exp
            && (number.ends_with('e')
                || number.ends_with('E')
                || number.ends_with('+')
                || number.ends_with('-'))
        {
            return Token::UnexpectedChar('e', self.line, self.column);
        }

        if has_dot || has_exp {
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
        if self.current_char.is_none() {
            return Token::UnterminatedString(self.line, self.column);
        }
        self.advance();

        Token::StringLit(value)
    }

    fn read_quoted_identifier(&mut self) -> Token {
        let mut value = String::new();

        self.advance();

        while let Some(ch) = self.current_char {
            if ch == '\"' && self.peek() == Some('\"') {
                value.push('\"');
                self.advance();
                self.advance();
            } else if ch == '\"' {
                break;
            } else {
                value.push(ch);
                self.advance();
            }
        }
        if self.current_char.is_none() {
            return Token::UnterminatedString(self.line, self.column);
        }
        self.advance();

        Token::QuotedIdent(value)
    }

    fn read_escape_string(&mut self) -> Token {
        let mut value = String::new();
        self.advance(); // consume 'E'
        self.advance(); // consume '\''

        while let Some(ch) = self.current_char {
            if ch == '\\' {
                self.advance();
                if let Some(esc) = self.current_char {
                    match esc {
                        'n' => value.push('\n'),
                        't' => value.push('\t'),
                        'r' => value.push('\r'),
                        '\\' => value.push('\\'),
                        '\'' => value.push('\''),
                        _ => {
                            value.push('\\');
                            value.push(esc);
                        }
                    }
                    self.advance();
                }
            } else if ch == '\'' && self.peek() == Some('\'') {
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

        if self.current_char.is_none() {
            return Token::UnterminatedString(self.line, self.column);
        }
        self.advance(); // consume closing '\''

        Token::StringLit(value)
    }

    fn read_backtick_identifier(&mut self) -> Token {
        let mut value = String::new();
        self.advance();

        while let Some(ch) = self.current_char {
            if ch == '`' && self.peek() == Some('`') {
                value.push('`');
                self.advance();
                self.advance();
            } else if ch == '`' {
                break;
            } else {
                value.push(ch);
                self.advance();
            }
        }
        if self.current_char.is_none() {
            return Token::UnterminatedString(self.line, self.column);
        }
        self.advance();

        Token::QuotedIdent(value)
    }

    fn read_bit_string(&mut self) -> Token {
        let mut value = String::new();
        self.advance(); // consume B
        self.advance(); // consume '

        while let Some(ch) = self.current_char {
            if ch == '\'' {
                break;
            }
            if ch != '0' && ch != '1' {
                return Token::UnexpectedChar(ch, self.line, self.column);
            }
            value.push(ch);
            self.advance();
        }
        if self.current_char.is_none() {
            return Token::UnterminatedString(self.line, self.column);
        }
        self.advance();

        Token::BitStringLit(value)
    }

    fn read_hex_string(&mut self) -> Token {
        let mut value = String::new();
        self.advance(); // consume X
        self.advance(); // consume '

        while let Some(ch) = self.current_char {
            if ch == '\'' {
                break;
            }

            if ch.is_ascii_hexdigit() {
                value.push(ch);
                self.advance();
            } else {
                return Token::UnexpectedChar(ch, self.line, self.column);
            }
        }
        if self.current_char.is_none() {
            return Token::UnterminatedString(self.line, self.column);
        }
        self.advance();

        Token::HexStringLit(value)
    }

    pub fn next_token(&mut self) -> Token {
        loop {
            self.skip_whitespace();

            let is_comment = (self.current_char == Some('-') && self.peek() == Some('-'))
                || (self.current_char == Some('/') && self.peek() == Some('*'));

            if is_comment {
                if let Err(err_token) = self.skip_comment() {
                    return err_token;
                }
            } else {
                break;
            }
        }

        if let Some(ch) = self.current_char {
            match ch {
                'E' | 'e' if self.peek() == Some('\'') => {
                    return self.read_escape_string();
                }
                'B' | 'b' if self.peek() == Some('\'') => {
                    return self.read_bit_string();
                }
                'X' | 'x' if self.peek() == Some('\'') => {
                    return self.read_hex_string();
                }
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
                '`' => {
                    return self.read_backtick_identifier();
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
                    } else if self.peek() == Some('~') {
                        self.advance();
                        self.advance();

                        if self.current_char == Some('*') {
                            self.advance();
                            return Token::RegexNotIMatch;
                        }

                        return Token::RegexNotMatch;
                    }

                    // let (line, col) = (self.line, self.column);
                    let (line, col) = self.current_position();
                    self.advance();
                    return Token::Illegal('!', line, col);
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
                    } else if self.peek() == Some('@') {
                        self.advance();
                        self.advance();
                        return Token::LtAt;
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

                '[' => {
                    self.advance();
                    return Token::LBracket;
                }

                ']' => {
                    self.advance();
                    return Token::RBracket;
                }

                '{' => {
                    self.advance();
                    return Token::LBrace;
                }

                '}' => {
                    self.advance();
                    return Token::RBrace;
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
                        return Token::UnexpectedChar('|', self.line, self.column);
                    }
                }

                ':' => {
                    if self.peek() == Some(':') {
                        self.advance();
                        self.advance();
                        return Token::DoubleColon;
                    } else {
                        self.advance();
                        return Token::UnexpectedChar(':', self.line, self.column);
                    }
                }

                '@' => {
                    if self.peek() == Some('>') {
                        self.advance();
                        self.advance();
                        return Token::AtGt;
                    } else if self.peek() == Some('@') {
                        self.advance();
                        self.advance();
                        return Token::AtAt;
                    }

                    self.advance();
                    return Token::At;
                }

                '#' => {
                    self.advance();

                    if self.current_char == Some('>') {
                        self.advance();

                        if self.current_char == Some('>') {
                            self.advance();
                            return Token::HashDoubleArrow;
                        }

                        return Token::HashArrow;
                    }

                    return Token::Hash;
                }

                '~' => {
                    self.advance();

                    if self.current_char == Some('*') {
                        self.advance();
                        return Token::RegexIMatch;
                    }

                    return Token::RegexMatch;
                }

                '$' => {
                    self.advance();

                    let mut digits = String::new();

                    while let Some(ch) = self.current_char {
                        if ch.is_ascii_digit() {
                            digits.push(ch);
                            self.advance();
                        } else {
                            break;
                        }
                    }

                    if digits.is_empty() {
                        return Token::Illegal('$', self.line, self.column);
                    }

                    // return Token::Parameter(digits.parse().unwrap());
                    return match digits.parse::<u32>() {
                        Ok(n) => Token::Parameter(n),
                        Err(_) => return Token::Illegal('$', self.line, self.column),
                    };
                }

                // unknown character
                _ => {
                    self.advance();
                    return Token::UnexpectedChar(ch, self.line, self.column);
                }
            }
        }

        Token::Eof
    }
}

impl Iterator for Lexer {
    type Item = Token;

    fn next(&mut self) -> Option<Token> {
        match self.next_token() {
            Token::Eof => None,
            t => Some(t),
        }
    }
}
