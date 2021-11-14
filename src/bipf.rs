use indexmap::IndexMap;
use serde_json::Value;
use integer_encoding::VarInt;
use either::*;
use bytes::{BytesMut, BufMut, Bytes};
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

pub trait Bipf {
    fn to_bipf(&self) -> Bytes;
}

impl Bipf for Value {
    fn to_bipf(&self) -> Bytes { 
        JType::new(self).encode()
    }
}

enum JType {
    String { v: String, l: usize },
    Int { v: i32 },
    Double { v: Either<i64, f64> },
    Array { v: Vec<JType>, l: usize },
    Object { v: IndexMap<String, JType>, l: usize },
    BoolNull { v: Option<bool>, l: usize }
}

impl JType {
    pub fn new(input: &Value) -> JType {
        match input {
            Value::Null => JType::BoolNull { v: None, l: 0 },
            Value::Bool(v) => JType::BoolNull { v: Some(*v), l: 1 },
            Value::String(s) => JType::String { l: s.len(), v: s.clone() },
            Value::Array(arr) => {
                let v: Vec<JType> = arr.into_iter().map(|x| JType::new(x)).collect();
                let l: usize = v.iter().map(|x| {
                    let l = JType::length(&x);
                    l + (l << TAG_SIZE).required_space()
                }).sum();
                JType::Array { v, l }
            },
            Value::Number(n) => {
                if n.is_i64() {
                    let i64 = n.as_i64().unwrap();
                    if i64.abs() < 4294967296 {
                        JType::Int { v: i64 as i32 }
                    } else {
                        JType::Double { v: Left(i64) }
                    }
                } else {
                    JType::Double { v: Right(n.as_f64().unwrap()) }
                }
            }
            Value::Object(o) => {
                let v: IndexMap<String, JType> = o.into_iter().map(|(k, v)| (k.clone(), JType::new(v))).collect();
                let l = v.iter().map(|(k, v)| {
                    let key_len = k.len();
                    let val_length = JType::length(v);
                    key_len + (key_len << TAG_SIZE).required_space() + val_length + (val_length << TAG_SIZE).required_space()
                }).sum();

                JType::Object { v, l }
            }
        }  
    }

    pub fn encode_rec(&self, buf: &mut BytesMut, start: usize) -> usize { 
        let length_varint = self.length() << TAG_SIZE | self.get_type();
        let varint = length_varint.encode_var_vec();
        let varint_length = varint.len();
        buf.put_slice(&varint);

        varint_length + match self {
            JType::String { v, l } => {
                buf.put_slice(v.as_bytes());
                *l
            },
            JType::Int { v } => {
                buf.put_slice(&v.to_le_bytes());
                4
            },
            JType::Double { v } => match v {
                Left(int) => {
                    buf.put_slice(&int.to_le_bytes());
                    8
                },
                Right(float) => {
                    buf.put_slice(&float.to_le_bytes());
                    8
                }
            },
            JType::BoolNull { v, l: _ } => {
                match v {
                    None => 0,
                    Some(b) => {
                        buf.put_u8(if *b { 1 } else { 0 });
                        1
                    }
                }
            }
            JType::Array { v, l: _ } => {
                let mut p = start;
                for i in v {
                    p += i.encode_rec(buf, p)
                }
                p - start
            },
            JType::Object { v, l: _ } => {
                let mut p = start;
                for (k, u) in v {
                    p += JType::String {v: k.clone(), l: k.len() }.encode_rec(buf, p);
                    p += u.encode_rec(buf, p);
                }
                p - start
            }
        }
    }

    pub fn encode(&self) -> Bytes {
        let mut buf: BytesMut = BytesMut::with_capacity(self.length());
        self.encode_rec(&mut buf, 0);

        buf.freeze()
    }

    pub fn length(&self) -> usize {
        match self {
            JType::String { v: _, l } => *l,
            JType::Int { v: _ } => 4,
            JType::Double { v: _ } => 8,
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

pub fn decode(buf: &Bytes) -> Result<Value> {
    decode_rec(buf, 0)
}

fn decode_rec(buf: &Bytes, start: usize) -> Result<Value> {
    let decoded: Option<(usize, usize)> = VarInt::decode_var(&buf[start..]);
    let (tag, bytes) = match decoded {
        None => Err(Error::from(ErrorKind::InvalidInput)),
        Some(v) => Result::Ok(v)
    }?;

    let field_type = tag & TAG_MASK;
    let len = tag >> TAG_SIZE;

    decode_type(field_type, buf, start + bytes, len)
}

fn decode_type(field_type: usize, buf: &Bytes, start: usize, len: usize) -> Result<Value>  {
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

fn decode_boolnull(buf: &Bytes, start: usize, len: usize) -> Result<Value> {
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

fn decode_string(buf: &Bytes, start: usize, len: usize) -> Result<Value> {
    let raw_str = std::str::from_utf8(&buf[start..start + len]);
    match raw_str {
        std::result::Result::Ok(v) => Ok(Value::String(String::from(v))),
        std::result::Result::Err(_) => Err(Error::new(ErrorKind::Other, "Could not decode utf-8 string"))
    }    
}

fn decode_integer(buf: &Bytes, start: usize) -> Result<Value> {
    let bytes: [u8; 4] = buf[start..start + 4].try_into().expect("slice with incorrect length");
    Ok(serde_json::to_value(i32::from_le_bytes(bytes))?)
}

fn decode_double(buf: &Bytes, start: usize) -> Result<Value> {
    let bytes: [u8; 8] = buf[start..start+8].try_into().expect("slice with incorrect length");
    Ok(serde_json::to_value(f64::from_le_bytes(bytes))?)
}

fn decode_array(buf: &Bytes, start: usize, len: usize) -> Result<Value> {
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

fn decode_object(buf: &Bytes, start: usize, len: usize) -> Result<Value> {
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
