use std::io;

#[derive(Debug, PartialEq, Eq)]
pub enum JsonType {
    None,
    String,
    Number,
    True,
    False,
    Null,
    Array,
    Object,
}

pub struct JsonParser<T, F> where
    T: std::io::Read,
    F: FnMut(&str, JsonType, &str) 
{
    reader: T,
    ungets: Vec<u8>,
    path :String,
    value: String,
    on_value: F,
}

impl<T, F> JsonParser<T, F> where
    T: std::io::Read, 
    F: FnMut(&str, JsonType, &str) 
{
    pub fn new(reader : T, on_value: F) -> Self {
        Self {
            reader,
            ungets: Vec::new(),
            path: String::from("$"),
            value: String::new(),
            on_value,
        }
    }

    fn getc(&mut self) -> Result<Option<u8>, String> {
        if let Some(u) = self.ungets.pop() {
            return Ok(Some(u));
        }

        let mut b: [u8; 1] = [0];
        match self.reader.read_exact(&mut b) {
            Ok(()) => Ok(Some(b[0])),
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn ungetc(&mut self, c: u8) {
        self.ungets.push(c);
    }

    fn read_value(&mut self) -> Result<(), String> {
        while let Some(c) = self.getc()? {
            match c {
                b'{' => {
                    self.read_object()?;
                    (self.on_value)(&self.path, JsonType::Object, &self.value);
                },
                b'[' => {
                    self.read_array()?;
                    (self.on_value)(&self.path, JsonType::Array, &self.value);
                },
                b'0' ..= b'9' | b'-' => {
                    self.ungetc(c);
                    self.read_number()?;
                    (self.on_value)(&self.path, JsonType::Number, &self.value);
                },
                b'"' => {
                    self.value.clear();
                    self.read_string(false)?;
                    (self.on_value)(&self.path, JsonType::String, &self.value);
                },
                b't' => {
                    self.read_literal(b"rue")?;
                    (self.on_value)(&self.path, JsonType::True, &self.value);
                },
                b'f' => {
                    self.read_literal(b"alse")?;
                    (self.on_value)(&self.path, JsonType::False, &self.value);
                },
                b'n' => {
                    self.read_literal(b"ull")?;
                    (self.on_value)(&self.path, JsonType::Null, &self.value);
                },
                b' ' | b'\t' | b'\r' | b'\n' => continue,
                _ => return Err("unexpected char".into()),
            }
        }
        Ok(())
    }

    fn read_number(&mut self) -> Result<(), String> {
        self.value.clear();

        while let Some(c) = self.getc()? {
            if !(c >= b'0' && c <= b'9') && c != b'.' && c != b'-' && c != b'+' && c != b'e' && c != b'E' {
                self.ungetc(c);
                break;
            } else {
                self.value.push(c as char);
            }
        }

        Ok(())
    }

    fn read_string(&mut self, path: bool) -> Result<usize, String> {
        let mut in_escape = false;
        let mut i: usize = 0;
        
        while let Some(c) = self.getc()? {
            match c {
                b'\\' => {
                    in_escape = true;
                },
                b'"' if !in_escape => {
                    return Ok(i);
                },
                _ => {
                    if in_escape {
                        in_escape = false;
                    }
                    if path {
                        self.path.push(c as char);
                    } else {
                        self.value.push(c as char);
                    }
                    i += 1;
                }
            }
        }
        Err("unterminated string".into())
    }

    fn read_literal(&mut self, s: &[u8]) -> Result<(), String> {
        for i in 0..s.len() {
            let b = self.getc()?;
            
            if let Some(x) = b {
                if x != s[i] {
                    return Err(format!("expected '{}' but got '{}'", s[i] as char, x as char));
                }
            } else {
                return Err(format!("unexpected end instead of '{}'", s[i] as char));
            }
        }
        
        Ok(())
    }

    fn read_array(&mut self) -> Result<(), String> {
        let mut reading_value = true;
        let mut i: usize = 0;
        
        while let Some(c) = self.getc()? {
            match c {
                b']' => return Ok(()),
                b',' => {
                    i += 1;
                    reading_value = true;
                },
                b' ' | b'\t' | b'\r' | b'\n' => continue,
                _ => {
                    if !reading_value {
                        return Err(format!("invalid char '{}'", c));
                    }

                    self.ungetc(c);
                    let l = self.path.len();
                    self.path.push_str(&format!("[{}]", i));

                    self.read_value()?;

                    self.path.truncate(l);
                    reading_value = false;
                }
            }
        }
        Err("unexpected end of array".into())
    }

    fn read_object(&mut self) -> Result<(), String> {
        let mut key_len = 0;
        self.path.push('.');
        
        while let Some(c) = self.getc()? {
            match c {
                b'}' => {
                    self.path.truncate(self.path.len() - key_len - 1);
                    return Ok(());
                },
                b'"' => {
                    if key_len != 0 {
                        return Err(format!("expecting a ':' after '{}'", self.path));
                    }
                    
                    key_len = self.read_string(true)?;
                },
                b':' => {
                    if key_len == 0 {
                        return Err("expecting a key before ':'".into());
                    }
                    self.read_value()?;
                },
                b',' => {
                    self.path.truncate(self.path.len() - key_len);
                    key_len = 0;
                },
                b' ' | b'\t' | b'\r' | b'\n' => continue,
                _ => return Err("unexpected char".into())
            }
        }
        Err("unexpected end of object".into())
    }
}

pub fn json_parse<T, F>(input: T, on_value: F) -> Result<(), String>
where
    T: std::io::Read, 
    F: FnMut(&str, JsonType, &str)
{
    JsonParser::new(input, on_value).read_value()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_number() {
        let data = b"1234.56";
        json_parse(data.as_slice(), |p, t, v| {
            assert_eq!("$", p);
            assert_eq!(JsonType::Number, t);
            assert_eq!("1234.56", v);
        }).expect("failed to parse");
    }
}