#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Regex {
    Empty,
    Char(char),
    Concat(Box<Regex>, Box<Regex>),
    Union(Box<Regex>, Box<Regex>),
    Star(Box<Regex>),
}

struct Parser<'a> {
    chars: std::str::Chars<'a>,
    lookahead: Option<char>,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        let mut chars = input.chars();
        let lookahead = chars.next();
        Parser { chars, lookahead }
    }

    fn advance(&mut self) {
        self.lookahead = self.chars.next();
    }

    fn parse_expr(&mut self) -> Result<Regex, String> {
        let left = self.parse_concat()?;
        if self.lookahead == Some('|') {
            self.advance();
            let right = self.parse_expr()?;
            Ok(Regex::Union(Box::new(left), Box::new(right)))
        } else {
            Ok(left)
        }
    }

    fn parse_concat(&mut self) -> Result<Regex, String> {
        let mut left = self.parse_factor()?;
        while self.lookahead.is_some_and(|char| char != '|' && char != ')') {
            let right = self.parse_factor()?;
            left = Regex::Concat(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_factor(&mut self) -> Result<Regex, String> {
        let atom = self.parse_atom()?;
        if self.lookahead == Some('*') {
            self.advance();
            Ok(Regex::Star(Box::new(atom)))
        } else {
            Ok(atom)
        }
    }

    fn parse_atom(&mut self) -> Result<Regex, String> {
        match self.lookahead {
            None => Ok(Regex::Empty),
            Some('(') => {
                self.advance();
                let expr = self.parse_expr()?;
                if self.lookahead != Some(')') {
                    return Err("Expected ')'".to_string());
                }
                self.advance();
                Ok(expr)
            }
            Some('|') | Some(')') | Some('*') => {
                Err(format!("Unexpected '{}'", self.lookahead.unwrap()))
            }
            Some(char) => {
                self.advance();
                Ok(Regex::Char(char))
            }
        }
    }
}

impl Regex {
    pub fn parse(input: &str) -> Result<Regex, String> {
        let mut parser = Parser::new(input);
        let result = parser.parse_expr()?;
        if parser.lookahead.is_some() {
            return Err(format!("Unexpected trailing character '{}'", parser.lookahead.unwrap()));
        }
        Ok(result)
    }
}