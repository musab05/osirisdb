use crate::lexer::{TokenKind, lookup_keyword::lookup_keyword, spanned_token::Span, token::Token};

/// A lexical analyzer for tokenizing SQL queries.
///
/// Converts a raw SQL query string into a stream of tokens with positional metadata
/// (line and column numbers) used by the parser.
pub struct Lexer<'a> {
    input: &'a [u8],
    position: usize,
    current_char: Option<u8>,
    line: usize,
    column: usize,
}

impl<'a> Lexer<'a> {
    /// Creates a new `Lexer` for the given SQL string.
    pub fn new(input: &'a str) -> Self {
        let bytes = input.as_bytes();

        Self {
            current_char: bytes.first().copied(),
            input: bytes,
            position: 0,
            line: 1,
            column: 1,
        }
    }

    /// Returns the current (line, column) position of the lexer (1-based).
    pub fn current_position(&self) -> (usize, usize) {
        (self.line, self.column)
    }

    fn advance(&mut self) {
        if let Some(ch) = self.current_char {
            if ch == b'\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
        }

        self.position += 1;

        self.current_char = self.input.get(self.position).copied();
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.position + 1).copied()
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.current_char {
            if ch.is_ascii_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn skip_comment(&mut self) -> Option<TokenKind> {
        if let Some(ch) = self.current_char {
            if ch == b'-' && self.peek() == Some(b'-') {
                self.advance();
                self.advance();

                while let Some(c) = self.current_char {
                    if c == b'\n' {
                        break;
                    }
                    self.advance();
                }
            } else if ch == b'/' && self.peek() == Some(b'*') {
                let mut depth = 1;

                self.advance();
                self.advance();

                while let Some(c) = self.current_char {
                    if c == b'/' && self.peek() == Some(b'*') {
                        depth += 1;
                        self.advance();
                        self.advance();
                    } else if c == b'*' && self.peek() == Some(b'/') {
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
                    return Some(TokenKind::UnterminatedComment);
                }
            }
        }
        None
    }

    fn read_identifier(&mut self) -> TokenKind {
        let start = self.position;

        while let Some(ch) = self.current_char {
            if ch.is_ascii_alphanumeric() || ch == b'_' {
                self.advance();
            } else {
                break;
            }
        }

        let word = std::str::from_utf8(&self.input[start..self.position]).unwrap_or("");
        lookup_keyword(word)
    }

    fn read_number(&mut self) -> TokenKind {
        let start = self.position;

        let mut has_dot = false;
        let mut has_exp = false;

        while let Some(ch) = self.current_char {
            if ch.is_ascii_digit() {
                self.advance();
            } else if ch == b'.' && !has_dot {
                if self.peek() == Some(b'.') {
                    break;
                }
                has_dot = true;
                self.advance();
            } else if (ch == b'e' || ch == b'E') && !has_exp {
                has_exp = true;
                self.advance();
                if self.current_char == Some(b'+') || self.current_char == Some(b'-') {
                    self.advance();
                }
            } else {
                break;
            }
        }

        let number = unsafe { std::str::from_utf8_unchecked(&self.input[start..self.position]) };

        if has_exp
            && (number.ends_with('e')
                || number.ends_with('E')
                || number.ends_with('+')
                || number.ends_with('-'))
        {
            return TokenKind::UnexpectedChar(b'e');
        }

        if has_dot || has_exp {
            match number.parse::<f64>() {
                Ok(f) => TokenKind::FloatLit(f),
                Err(_) => TokenKind::UnexpectedChar(b'0'),
            }
        } else {
            match number.parse::<i64>() {
                Ok(i) => TokenKind::IntLit(i),
                Err(_) => TokenKind::UnexpectedChar(b'0'),
            }
        }
    }

    fn read_string(&mut self) -> TokenKind {
        self.advance();

        while let Some(ch) = self.current_char {
            if ch == b'\'' && self.peek() == Some(b'\'') {
                self.advance();
                self.advance();
            } else if ch == b'\'' {
                break;
            } else {
                self.advance();
            }
        }
        if self.current_char.is_none() {
            return TokenKind::UnterminatedString;
        }
        self.advance();

        TokenKind::StringLit
    }

    fn read_quoted_identifier(&mut self) -> TokenKind {
        self.advance();

        while let Some(ch) = self.current_char {
            if ch == b'\"' && self.peek() == Some(b'\"') {
                self.advance();
                self.advance();
            } else if ch == b'\"' {
                break;
            } else {
                self.advance();
            }
        }
        if self.current_char.is_none() {
            return TokenKind::UnterminatedString;
        }
        self.advance();

        TokenKind::QuotedIdent
    }

    fn read_escape_string(&mut self) -> TokenKind {
        self.advance(); // consume 'E'
        self.advance(); // consume '\''

        while let Some(ch) = self.current_char {
            if ch == b'\\' {
                self.advance();
                if self.current_char.is_some() {
                    self.advance();
                }
            } else if ch == b'\'' && self.peek() == Some(b'\'') {
                self.advance();
                self.advance();
            } else if ch == b'\'' {
                break;
            } else {
                self.advance();
            }
        }

        if self.current_char.is_none() {
            return TokenKind::UnterminatedString;
        }
        self.advance(); // consume closing '\''

        TokenKind::StringLit
    }

    fn read_backtick_identifier(&mut self) -> TokenKind {
        self.advance();

        while let Some(ch) = self.current_char {
            if ch == b'`' && self.peek() == Some(b'`') {
                self.advance();
                self.advance();
            } else if ch == b'`' {
                break;
            } else {
                self.advance();
            }
        }
        if self.current_char.is_none() {
            return TokenKind::UnterminatedString;
        }
        self.advance();

        TokenKind::QuotedIdent
    }

    fn read_bit_string(&mut self) -> TokenKind {
        self.advance(); // consume B
        self.advance(); // consume '

        while let Some(ch) = self.current_char {
            if ch == b'\'' {
                break;
            }
            if ch != b'0' && ch != b'1' {
                return TokenKind::UnexpectedChar(ch);
            }
            self.advance();
        }
        if self.current_char.is_none() {
            return TokenKind::UnterminatedString;
        }
        self.advance();

        TokenKind::BitStringLit
    }

    fn read_hex_string(&mut self) -> TokenKind {
        self.advance(); // consume X
        self.advance(); // consume '

        while let Some(ch) = self.current_char {
            if ch == b'\'' {
                break;
            }

            if ch.is_ascii_hexdigit() {
                self.advance();
            } else {
                return TokenKind::UnexpectedChar(ch);
            }
        }
        if self.current_char.is_none() {
            return TokenKind::UnterminatedString;
        }
        self.advance();

        TokenKind::HexStringLit
    }

    fn next_token_kind(&mut self) -> TokenKind {
        if let Some(ch) = self.current_char {
            match ch {
                b'E' | b'e' if self.peek() == Some(b'\'') => {
                    return self.read_escape_string();
                }
                b'B' | b'b' if self.peek() == Some(b'\'') => {
                    return self.read_bit_string();
                }
                b'X' | b'x' if self.peek() == Some(b'\'') => {
                    return self.read_hex_string();
                }
                // Identifier or Keywords
                c if c.is_ascii_alphabetic() || c == b'_' => {
                    return self.read_identifier();
                }
                // Numbers
                c if c.is_ascii_digit() => {
                    return self.read_number();
                }
                // Strings
                b'\'' => {
                    return self.read_string();
                }
                b'\"' => {
                    return self.read_quoted_identifier();
                }
                b'`' => {
                    return self.read_backtick_identifier();
                }
                // Operators
                b'=' => {
                    self.advance();
                    return TokenKind::Eq;
                }
                b'!' => {
                    if self.peek() == Some(b'=') {
                        self.advance();
                        self.advance();
                        return TokenKind::Ne;
                    } else if self.peek() == Some(b'~') {
                        self.advance();
                        self.advance();

                        if self.current_char == Some(b'*') {
                            self.advance();
                            return TokenKind::RegexNotIMatch;
                        }

                        return TokenKind::RegexNotMatch;
                    }

                    self.advance();
                    return TokenKind::Illegal(b'!');
                }
                b'<' => {
                    if self.peek() == Some(b'=') {
                        self.advance();
                        self.advance();
                        return TokenKind::Le;
                    } else if self.peek() == Some(b'>') {
                        self.advance();
                        self.advance();
                        return TokenKind::Ne;
                    } else if self.peek() == Some(b'@') {
                        self.advance();
                        self.advance();
                        return TokenKind::LtAt;
                    }

                    self.advance();
                    return TokenKind::Lt;
                }

                b'>' => {
                    if self.peek() == Some(b'=') {
                        self.advance();
                        self.advance();
                        return TokenKind::Ge;
                    }

                    self.advance();
                    return TokenKind::Gt;
                }

                b'+' => {
                    self.advance();
                    return TokenKind::Plus;
                }

                b'-' => {
                    if self.peek() == Some(b'>') {
                        self.advance();
                        if self.peek() == Some(b'>') {
                            self.advance();
                            self.advance();
                            return TokenKind::DoubleArrow;
                        } else {
                            self.advance();
                            return TokenKind::Arrow;
                        }
                    } else {
                        self.advance();
                        return TokenKind::Minus;
                    }
                }

                b'*' => {
                    self.advance();
                    return TokenKind::Star;
                }

                b'/' => {
                    self.advance();
                    return TokenKind::Slash;
                }

                b'%' => {
                    self.advance();
                    return TokenKind::Percent;
                }

                // punctuation
                b'(' => {
                    self.advance();
                    return TokenKind::LParen;
                }

                b')' => {
                    self.advance();
                    return TokenKind::RParen;
                }

                b'[' => {
                    self.advance();
                    return TokenKind::LBracket;
                }

                b']' => {
                    self.advance();
                    return TokenKind::RBracket;
                }

                b'{' => {
                    self.advance();
                    return TokenKind::LBrace;
                }

                b'}' => {
                    self.advance();
                    return TokenKind::RBrace;
                }

                b',' => {
                    self.advance();
                    return TokenKind::Comma;
                }

                b';' => {
                    self.advance();
                    return TokenKind::Semicolon;
                }

                b'.' => {
                    self.advance();
                    return TokenKind::Dot;
                }

                b'|' => {
                    if self.peek() == Some(b'|') {
                        self.advance();
                        self.advance();
                        return TokenKind::Concat;
                    } else {
                        self.advance();
                        return TokenKind::UnexpectedChar(b'|');
                    }
                }

                b':' => {
                    if self.peek() == Some(b':') {
                        self.advance();
                        self.advance();
                        return TokenKind::DoubleColon;
                    } else {
                        self.advance();
                        return TokenKind::UnexpectedChar(b':');
                    }
                }

                b'@' => {
                    if self.peek() == Some(b'>') {
                        self.advance();
                        self.advance();
                        return TokenKind::AtGt;
                    } else if self.peek() == Some(b'@') {
                        self.advance();
                        self.advance();
                        return TokenKind::AtAt;
                    }

                    self.advance();
                    return TokenKind::At;
                }

                b'#' => {
                    self.advance();

                    if self.current_char == Some(b'>') {
                        self.advance();

                        if self.current_char == Some(b'>') {
                            self.advance();
                            return TokenKind::HashDoubleArrow;
                        }

                        return TokenKind::HashArrow;
                    }

                    return TokenKind::HashToken;
                }

                b'~' => {
                    self.advance();

                    if self.current_char == Some(b'*') {
                        self.advance();
                        return TokenKind::RegexIMatch;
                    }

                    return TokenKind::RegexMatch;
                }

                b'$' => {
                    // Check if it's a dollar-quoted string lit ($$ or $tag$) or a parameter ($1, $2, etc.)
                    if self.peek() == Some(b'$') {
                        // Empty tag: $$
                        self.advance(); // consume first '$'
                        self.advance(); // consume second '$'
                        // Now scan for the closing '$$'
                        while let Some(ch) = self.current_char {
                            if ch == b'$' && self.peek() == Some(b'$') {
                                self.advance(); // consume closing first '$'
                                self.advance(); // consume closing second '$'
                                return TokenKind::DollarStringLit;
                            }
                            self.advance();
                        }
                        return TokenKind::UnterminatedString;
                    } else if self
                        .peek()
                        .map_or(false, |c| c.is_ascii_alphabetic() || c == b'_')
                    {
                        // Tagged dollar-quoted string: e.g. $tag$
                        self.advance(); // consume first '$'
                        let mut tag = vec![];
                        while let Some(ch) = self.current_char {
                            if ch == b'$' {
                                break;
                            }
                            if ch.is_ascii_alphanumeric() || ch == b'_' {
                                tag.push(ch);
                                self.advance();
                            } else {
                                // Not a valid tag character. Fallback to parameter check or illegal.
                                return TokenKind::Illegal(b'$');
                            }
                        }
                        if self.current_char != Some(b'$') {
                            return TokenKind::UnterminatedString;
                        }
                        self.advance(); // consume second '$' of start tag

                        // Now search for the closing tag: $ + tag + $
                        let tag_len = tag.len();
                        loop {
                            if self.current_char.is_none() {
                                return TokenKind::UnterminatedString;
                            }
                            if self.current_char == Some(b'$') {
                                // Check if the following characters match tag + $
                                let mut matches = true;
                                for i in 0..tag_len {
                                    if self.input.get(self.position + 1 + i).copied()
                                        != Some(tag[i])
                                    {
                                        matches = false;
                                        break;
                                    }
                                }
                                if matches
                                    && self.input.get(self.position + 1 + tag_len).copied()
                                        == Some(b'$')
                                {
                                    // Found the closing tag!
                                    self.advance(); // consume '$'
                                    for _ in 0..tag_len {
                                        self.advance();
                                    }
                                    self.advance(); // consume closing '$'
                                    return TokenKind::DollarStringLit;
                                }
                            }
                            self.advance();
                        }
                    } else {
                        // Parameter: $1, $2, etc.
                        self.advance(); // consume '$'
                        let mut digits = String::new();
                        while let Some(ch) = self.current_char {
                            if ch.is_ascii_digit() {
                                digits.push(ch as char);
                                self.advance();
                            } else {
                                break;
                            }
                        }
                        if digits.is_empty() {
                            return TokenKind::Illegal(b'$');
                        }
                        return match digits.parse::<u32>() {
                            Ok(n) => TokenKind::Parameter(n),
                            Err(_) => TokenKind::Illegal(b'$'),
                        };
                    }
                }

                // unknown character
                _ => {
                    self.advance();
                    return TokenKind::UnexpectedChar(ch);
                }
            }
        }

        TokenKind::Eof
    }

    /// Advances the lexer and returns the next spanned token.
    ///
    /// Skips all whitespace and comments automatically. Returns a token of kind
    /// [`TokenKind::Eof`] when the end of the input is reached.
    pub fn next_token(&mut self) -> Token {
        let kind = loop {
            self.skip_whitespace();
            let is_comment = (self.current_char == Some(b'-') && self.peek() == Some(b'-'))
                || (self.current_char == Some(b'/') && self.peek() == Some(b'*'));

            if is_comment {
                if let Some(err_token) = self.skip_comment() {
                    break err_token;
                }
            } else {
                let start = self.position;
                let line = self.line;
                let column = self.column;

                let kind = self.next_token_kind();

                let end = self.position;

                return Token {
                    kind,
                    span: Span {
                        start,
                        end,
                        line,
                        column,
                    },
                };
            }
        };

        // If we break with err_token
        let start = self.position;
        let end = self.position;
        Token {
            kind,
            span: Span {
                start,
                end,
                line: self.line,
                column: self.column,
            },
        }
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Token;

    fn next(&mut self) -> Option<Token> {
        let token = self.next_token();
        match token.kind {
            TokenKind::Eof => None,
            _ => Some(token),
        }
    }
}
