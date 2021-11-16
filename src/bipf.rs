use indexmap::IndexMap;
use serde_json::Value;
use integer_encoding::VarInt;
use either::*;
use std::io::*;

const STRING: usize = 0;    // 000
//const BUFFER: u64 = 1;    // 001
const INT: usize = 2;       // 010 //32bit int
const DOUBLE: usize = 3;    // 011 //use next 8 bytes to encode 64bit float
const ARRAY: usize = 4;     // 100
const OBJECT: usize = 5;    // 101
const BOOLNULL: usize = 6;  // 110 //and use the rest of the byte as true/false/null
// const RESERVED: u64 = 7;  // 111

const TAG_SIZE: usize = 3;
const TAG_MASK: usize = 7;

const JSON_INT_SIZE: usize = 4;
const JSON_DOUBLE_SIZE: usize = 4;
const JSON_BOOL_SIZE: usize = 1;
const JSON_NULL_SIZE: usize = 0;

const MAX_I32: i64 = 4294967296;

pub trait Bipf {
    fn to_bipf(&self) -> Result<Vec<u8>>;
}

impl Bipf for Value {
    fn to_bipf(&self) -> Result<Vec<u8>> {
        JType::new(self).encode()
    }
}

enum JType<'a> {
    String { v: &'a String, l: usize },
    Int { v: i32 },
    Double { v: Either<i64, f64> },
    Array { v: Vec<JType<'a>>, l: usize },
    Object { v: IndexMap<&'a String, JType<'a>>, l: usize },
    BoolNull { v: Option<bool>, l: usize }
}

impl<'a> JType<'a> {
    pub fn new(input: &'a Value) -> JType {
        match input {
            Value::Null => JType::BoolNull { v: None, l: JSON_NULL_SIZE },
            Value::Bool(v) => JType::BoolNull { v: Some(*v), l: JSON_BOOL_SIZE },
            Value::String(s) => JType::String { l: s.len(), v: s },
            Value::Array(arr) => {
                let v: Vec<JType> = arr.iter().map(|x| JType::new(x)).collect();
                let mut l = 0;
                for x in &v {
                    let base_len = JType::length(&x);
                    l += base_len + (base_len << TAG_SIZE).required_space()
                }
                JType::Array { v, l }
            },
            Value::Number(n) => {
                if n.is_i64() {
                    let i64 = n.as_i64().unwrap();
                    if i64.abs() < MAX_I32 {
                        JType::Int { v: i64 as i32 }
                    } else {
                        JType::Double { v: Left(i64) }
                    }
                } else {
                    JType::Double { v: Right(n.as_f64().unwrap()) }
                }
            },
            Value::Object(o) => {
                let v: IndexMap<&'a String, JType> = o.iter().map(|(k, v)| (k, JType::new(v))).collect();
                let mut l = 0;
                for (k, v) in &v {
                    let key_len = k.len();
                    let val_length = JType::length(&v);
                    l += key_len + (key_len << TAG_SIZE).required_space() + val_length + (val_length << TAG_SIZE).required_space()
                }
                JType::Object { v, l }
            }
        }
    }

    pub fn encode_rec(&self, buf: &mut Vec<u8>, start: usize) -> Result<usize> {
        let length_varint = self.length() << TAG_SIZE | self.get_type();
        let varint = length_varint.encode_var_vec();
        let varint_length = varint.len();
        buf.write(&varint)?;

        Ok(varint_length + (match self {
            JType::String { v, l: _ } => {
                buf.write(v.as_bytes())
            },
            JType::Int { v } => {
                buf.write(&v.to_le_bytes())
            },
            JType::Double { v: Left(int) } => {
                buf.write(&int.to_le_bytes())
            },
            JType::Double { v: Right(float) } => {
                buf.write(&float.to_le_bytes())
            },
            JType::BoolNull { v, l: _ } => {
                match v {
                    None => Ok(JSON_NULL_SIZE),
                    Some(b) => {
                        buf.write(&[if *b { 1 } else { 0 }])
                    }
                }
            }
            JType::Array { v, l: _ } => {
                let mut p = start;
                for i in v {
                    p += i.encode_rec(buf, p)?
                }
                Ok(p - start)
            },
            JType::Object { v, l: _ } => {
                let mut p = start;
                for (k, u) in v {
                    p += JType::String {v: k, l: k.len() }.encode_rec(buf, p)?;
                    p += u.encode_rec(buf, p)?;
                }
                Ok(p - start)
            }
        })?)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut buf: Vec<u8> = Vec::with_capacity(self.length());
        self.encode_rec(&mut buf, 0)?;

        buf.flush()?;

        Ok(buf)
    }

    pub fn length(&self) -> usize {
        match self {
            JType::String { v: _, l } => *l,
            JType::Int { v: _ } => JSON_INT_SIZE,
            JType::Double { v: _ } => JSON_DOUBLE_SIZE,
            JType::Array { v: _, l } => *l,
            JType::Object { v: _, l } => *l,
            JType::BoolNull { v: _, l } => *l,
        }
    }

    pub fn get_type(&self) -> usize {
        match self {
            JType::String { v: _, l: _ } => STRING,
            JType::Int { v: _ } => INT,
            JType::Double { v: _ } => DOUBLE,
            JType::Array { v: _, l: _ } => ARRAY,
            JType::Object { v: _, l: _ } => OBJECT,
            JType::BoolNull { v: _, l: _ } => BOOLNULL,
        }
    }
}

pub fn decode(buf: &Vec<u8>) -> Result<Value> {
    decode_rec(buf, 0)
}

pub fn decode_rec(buf: &Vec<u8>, start: usize) -> Result<Value> {
    let decoded: Option<(usize, usize)> = VarInt::decode_var(&buf[start..]);
    let (tag, bytes) = match decoded {
        None => Err(Error::from(ErrorKind::InvalidInput)),
        Some(v) => Result::Ok(v)
    }?;

    let field_type = tag & TAG_MASK;
    let len = tag >> TAG_SIZE;

    decode_type(field_type, buf, start + bytes, len)
}

pub fn decode_type(field_type: usize, buf: &Vec<u8>, start: usize, len: usize) -> Result<Value>  {
    match field_type {
        STRING => decode_string(buf, start, len),
        BOOLNULL => decode_boolnull(buf, start, len),
        INT => decode_integer(buf, start),
        DOUBLE => decode_double(buf, start),
        ARRAY => decode_array(buf, start, len),
        OBJECT => decode_object(buf, start, len),
        _ => Err(Error::new(ErrorKind::Other, "invalid type"))
    }
}

pub fn decode_boolnull(buf: &Vec<u8>, start: usize, len: usize) -> Result<Value> {
    if len == 0 {
        Ok(Value::Null)
    } else {
        if buf[start] > 2 {
            Err(Error::new(ErrorKind::Other, "Invalid boolnull"))
        } else {
            if len > 1 {
                Err(Error::new(ErrorKind::Other, "Invalid boolnull, len must be > 1"))
            } else {
                Ok(Value::Bool(if buf[start] == 1 {
                    true 
                } else { false }))
            }
        }
    }
}

pub fn decode_string(buf: &Vec<u8>, start: usize, len: usize) -> Result<Value> {
    let raw_str = std::str::from_utf8(&buf[start..start + len]);
    match raw_str {
        std::result::Result::Ok(v) => Ok(Value::String(String::from(v))),
        std::result::Result::Err(_) => Err(Error::new(ErrorKind::Other, "Could not decode utf-8 string"))
    }    
}

pub fn decode_integer(buf: &Vec<u8>, start: usize) -> Result<Value> {
    let bytes: [u8; 4] = buf[start..start + 4].try_into().expect("slice with incorrect length");
    Ok(serde_json::to_value(i32::from_le_bytes(bytes))?)
}

pub fn decode_double(buf: &Vec<u8>, start: usize) -> Result<Value> {
    let bytes: [u8; 8] = buf[start..start+8].try_into().expect("slice with incorrect length");
    Ok(serde_json::to_value(f64::from_le_bytes(bytes))?)
}

pub fn decode_array(buf: &Vec<u8>, start: usize, len: usize) -> Result<Value> {
    let mut c = 0;
    let mut vec: Vec<Value> = Vec::new();
    
    while c < len {
        let decoded: Option<(usize, usize)> = VarInt::decode_var(&buf[start + c..]);
        let (tag, bytes) = match decoded {
            None => Result::Err(Error::from(ErrorKind::InvalidInput)),
            Some(v) => Result::Ok(v)
        }?;

        c += bytes;

        let field_type = tag & TAG_MASK;
        let len = tag >> TAG_SIZE;

        vec.push(decode_type(field_type, buf, start + c, len)?);

        c += len;
    }

    Ok(Value::Array(vec))
}

pub fn decode_object(buf: &Vec<u8>, start: usize, len: usize) -> Result<Value> {
    let mut c = 0;
    let mut map: serde_json::Map<String, Value> = serde_json::Map::new();
    
    while c < len {
        let decoded: Option<(usize, usize)> = VarInt::decode_var(&buf[start + c..]);
        let (tag, bytes) = match decoded {
            None => Result::Err(Error::from(ErrorKind::InvalidInput)),
            Some(v) => Result::Ok(v)
        }?;
        c += bytes;
        let len = tag >> TAG_SIZE;
        let key: String = match decode_string(buf, start + c, len)? {
            Value::String(key) => Ok(key),
            _ => Err(Error::from(ErrorKind::InvalidInput))
        }?;
        c += len;

        let decoded: Option<(usize, usize)> = VarInt::decode_var(&buf[start + c..]);
        let (tag, bytes) = match decoded {
            None => Err(Error::from(ErrorKind::InvalidInput)),
            Some(v) => Result::Ok(v)
        }?;

        let field_type = tag & TAG_MASK;
        let len = tag >> TAG_SIZE;

        c += bytes;
        let value = decode_type(field_type, buf, start + c, len)?;
        c += len;
        map.insert(key, value);
    }

    Ok(Value::Object(map))
}

pub fn seek_key(bytes: &Vec<u8>, start: Option<usize>, target: String) -> Option<usize> {
    match start {
        None => None,
        Some(start) => {
            let decoded: (usize, usize) = VarInt::decode_var(&bytes[start..])?;
            let tag = decoded.0;
            let ty = tag & TAG_MASK;

            if ty != OBJECT {
                None
            } else {
                let mut c = decoded.1;
                let len = tag >> TAG_SIZE;
                let target_length = target.len();
                let target_buf = target.as_bytes();
                while c < len {
                    let key_tag: (usize, usize) = VarInt::decode_var(&bytes[start + c..])?;
                    c += key_tag.1;
                    let key_len = key_tag.0 >> TAG_SIZE;
                    let key_type = key_tag.0 & TAG_MASK;

                    if key_type == STRING && target_length == key_len {
                        if target_buf.eq(&bytes[start + c..start + c + target_length]) {
                            let next_start = start + c + key_len;
                            return Some(next_start)
                        }
                    }

                    c += key_len;
                    let value_tag: (usize, usize) = VarInt::decode_var(&bytes[start + c..])?;
                    c += value_tag.1;
                    let value_len = value_tag.0 >> TAG_SIZE;
                    c += value_len;
                }

                None
            }
        }
    }
}
