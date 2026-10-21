//! JSON mínimo para salvar o estado (o núcleo não usa bibliotecas externas).
//! Suporta objetos, listas, textos, inteiros, booleanos e null — só o necessário.

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Object(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Json::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Json::Array(items) => Some(items),
            _ => None,
        }
    }

    /// Texto com um campo por linha (listas curtas ficam numa linha só).
    pub fn write(&self) -> String {
        let mut out = String::new();
        write_value(self, 0, &mut out);
        out.push('\n');
        out
    }
}

fn write_value(value: &Json, indent: usize, out: &mut String) {
    match value {
        Json::Null => out.push_str("null"),
        Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Json::Int(n) => out.push_str(&n.to_string()),
        Json::Str(s) => write_string(s, out),
        Json::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_value(item, indent + 1, out);
            }
            out.push(']');
        }
        Json::Object(fields) => {
            out.push_str("{\n");
            for (i, (key, item)) in fields.iter().enumerate() {
                out.push_str(&"  ".repeat(indent + 1));
                write_string(key, out);
                out.push_str(": ");
                write_value(item, indent + 1, out);
                if i + 1 < fields.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&"  ".repeat(indent));
            out.push('}');
        }
    }
}

fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

pub(crate) fn parse(text: &str) -> Result<Json, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut parser = Parser { chars: &chars, pos: 0 };
    let value = parser.value()?;
    parser.skip_spaces();
    if parser.pos != chars.len() {
        return Err(format!("texto extra na posição {}", parser.pos));
    }
    Ok(value)
}

struct Parser<'a> {
    chars: &'a [char],
    pos: usize,
}

impl Parser<'_> {
    fn skip_spaces(&mut self) {
        while self.pos < self.chars.len() && self.chars[self.pos].is_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.skip_spaces();
        self.chars.get(self.pos).copied()
    }

    fn expect(&mut self, c: char) -> Result<(), String> {
        if self.peek() == Some(c) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!("esperado '{c}' na posição {}", self.pos))
        }
    }

    fn literal(&mut self, word: &str, value: Json) -> Result<Json, String> {
        let end = self.pos + word.len();
        if end <= self.chars.len() && self.chars[self.pos..end].iter().collect::<String>() == word {
            self.pos = end;
            Ok(value)
        } else {
            Err(format!("valor inválido na posição {}", self.pos))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        match self.peek() {
            Some('{') => self.object(),
            Some('[') => self.array(),
            Some('"') => Ok(Json::Str(self.string()?)),
            Some('t') => self.literal("true", Json::Bool(true)),
            Some('f') => self.literal("false", Json::Bool(false)),
            Some('n') => self.literal("null", Json::Null),
            Some(c) if c == '-' || c.is_ascii_digit() => self.number(),
            _ => Err(format!("valor inválido na posição {}", self.pos)),
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.expect('{')?;
        let mut fields = Vec::new();
        if self.peek() == Some('}') {
            self.pos += 1;
            return Ok(Json::Object(fields));
        }
        loop {
            if self.peek() != Some('"') {
                return Err(format!("esperada chave na posição {}", self.pos));
            }
            let key = self.string()?;
            self.expect(':')?;
            fields.push((key, self.value()?));
            match self.peek() {
                Some(',') => self.pos += 1,
                Some('}') => {
                    self.pos += 1;
                    return Ok(Json::Object(fields));
                }
                _ => return Err(format!("esperado ',' ou '}}' na posição {}", self.pos)),
            }
        }
    }

    fn array(&mut self) -> Result<Json, String> {
        self.expect('[')?;
        let mut items = Vec::new();
        if self.peek() == Some(']') {
            self.pos += 1;
            return Ok(Json::Array(items));
        }
        loop {
            items.push(self.value()?);
            match self.peek() {
                Some(',') => self.pos += 1,
                Some(']') => {
                    self.pos += 1;
                    return Ok(Json::Array(items));
                }
                _ => return Err(format!("esperado ',' ou ']' na posição {}", self.pos)),
            }
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect('"')?;
        let mut out = String::new();
        while let Some(&c) = self.chars.get(self.pos) {
            self.pos += 1;
            match c {
                '"' => return Ok(out),
                '\\' => {
                    let escaped = *self.chars.get(self.pos).ok_or("texto incompleto")?;
                    self.pos += 1;
                    match escaped {
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        '/' => out.push('/'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'u' => {
                            let hex: String = self.chars.get(self.pos..self.pos + 4).ok_or("\\u incompleto")?.iter().collect();
                            self.pos += 4;
                            let code = u32::from_str_radix(&hex, 16).map_err(|_| "\\u inválido")?;
                            out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                        }
                        _ => return Err(format!("escape inválido na posição {}", self.pos)),
                    }
                }
                c => out.push(c),
            }
        }
        Err("texto sem aspas finais".to_string())
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.pos;
        if self.chars.get(self.pos) == Some(&'-') {
            self.pos += 1;
        }
        while self.chars.get(self.pos).is_some_and(|c| c.is_ascii_digit()) {
            self.pos += 1;
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        text.parse().map(Json::Int).map_err(|_| format!("número inválido na posição {start}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_e_volta() {
        let value = Json::Object(vec![
            ("versao".into(), Json::Int(1)),
            ("nome".into(), Json::Str("a \"b\"\n".into())),
            ("lista".into(), Json::Array(vec![Json::Bool(true), Json::Null, Json::Int(-5)])),
            ("vazio".into(), Json::Object(vec![])),
        ]);
        assert_eq!(parse(&value.write()), Ok(value));
    }

    #[test]
    fn erros() {
        assert!(parse("{").is_err());
        assert!(parse("[1,]").is_err());
        assert!(parse("{\"a\": 1} x").is_err());
        assert!(parse("1.5").is_err());
    }
}
