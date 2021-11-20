use crate::bipf::*;
use integer_encoding::VarInt;
use neon::prelude::*;
use std::io::*;

pub fn encode_neon<'a>(mut cx: FunctionContext<'a>) -> JsResult<'a, JsArrayBuffer> {
    let val = cx.argument::<JsValue>(0)?;
    match JType::new(val, &mut cx) {
        Ok(val) => match val.encode(&mut cx) {
            Ok(res) => {
                // Todo write continuously
                let mut buf = JsArrayBuffer::new(&mut cx, res.len() as u32)?;
                let mut out = cx.borrow_mut(&mut buf, |x| x.as_mut_slice::<u8>());
                out.write(&res[..]);

                Ok(buf)
            }
            Err(_) => NeonResult::Err(neon::result::Throw),
        },
        Err(_) => NeonResult::Err(neon::result::Throw),
    }
}

enum JType<'a> {
    String {
        v: Handle<'a, JsString>,
        l: usize,
    },
    Buffer {
        v: Vec<u8>,
    },
    Number {
        v: f64,
    },
    Array {
        v: Vec<JType<'a>>,
        l: usize,
    },
    Object {
        v: Vec<(Handle<'a, JsString>, JType<'a>)>,
        l: usize,
    },
    BoolNull {
        v: Option<bool>,
        l: usize,
    },
}

impl<'a> JType<'a> {
    pub fn new(input: Handle<'a, JsValue>, cx: &mut FunctionContext<'a>) -> Result<JType<'a>> {
        if input.is_a::<JsNull, _>(cx) {
            Ok(JType::BoolNull {
                v: None,
                l: JSON_NULL_SIZE,
            })
        } else if input.is_a::<JsBoolean, _>(cx) {
            let res = match input.downcast::<JsBoolean, _>(cx) {
                Ok(b) => Ok(b.value(cx)),
                _ => Err(Error::new(ErrorKind::Other, "")),
            }?;
            Ok(JType::BoolNull {
                v: Some(res),
                l: JSON_BOOL_SIZE,
            })
        } else if input.is_a::<JsString, _>(cx) {
            let res = match input.downcast::<JsString, _>(cx) {
                Ok(b) => Ok(b),
                _ => Err(Error::new(ErrorKind::Other, "")),
            }?;

            Ok(JType::String {
                v: res,
                l: res.size(cx) as usize,
            })
        } else if input.is_a::<JsNumber, _>(cx) {
            // TODO properly handle numbers
            // https://medium.com/angular-in-depth/javascripts-number-type-8d59199db1b6#.9whwe88tz
            Ok(JType::Number {
                v: match input.downcast::<JsNumber, _>(cx) {
                    Ok(x) => Ok(x.value(cx)),
                    Err(_) => Err(Error::new(ErrorKind::Other, "")),
                }?,
            })
        } else if input.is_a::<JsBuffer, _>(cx) {
            match input.downcast::<JsBuffer, _>(cx) {
                Ok(b) => Ok(JType::Buffer {
                    v: cx.borrow(&b, |x| x.as_slice().to_vec()),
                }),
                Err(_) => Err(Error::new(ErrorKind::Other, "")),
            }
        } else if input.is_a::<JsArray, _>(cx) {
            let v: Vec<JType> = match input.downcast::<JsArray, _>(cx) {
                Ok(res) => match res.to_vec(cx) {
                    Ok(vec) => vec.iter().map(|x| JType::new(*x, cx)).collect(),
                    Err(_) => Err(Error::new(ErrorKind::Other, "")),
                },
                Err(_) => Err(Error::new(ErrorKind::Other, "")),
            }?;

            let mut l = 0;
            for x in &v {
                let base_len = JType::length(&x);
                l += base_len + (base_len << TAG_SIZE).required_space()
            }

            Ok(JType::Array { v, l })
        } else if input.is_a::<JsObject, _>(cx) {
            let res = match input.downcast::<JsObject, _>(cx) {
                Ok(b) => Ok(b),
                _ => Err(Error::new(ErrorKind::Other, "")),
            }?;

            let v: Vec<(Handle<'a, JsString>, JType<'a>)> =
                match res.get_own_property_names(cx).unwrap().to_vec(cx) {
                    Ok(vec) => vec
                        .iter()
                        .map(|x| match x.downcast::<JsString, _>(cx) {
                            Ok(s) => {
                                let inner = JType::new(
                                    match res.get(cx, s) {
                                        Ok(s) => Ok(s),
                                        _ => Err(Error::new(ErrorKind::Other, "")),
                                    }?,
                                    cx,
                                )?;

                                Ok((s, inner))
                            }
                            _ => Err(Error::new(ErrorKind::Other, "")),
                        })
                        .collect(),
                    Err(e) => Err(Error::new(ErrorKind::Other, "")),
                }?;

            let mut l = 0;
            for (k, v) in &v {
                let key_len = k.size(cx) as usize;
                let val_length = JType::length(&v);
                l += key_len
                    + (key_len << TAG_SIZE).required_space()
                    + val_length
                    + (val_length << TAG_SIZE).required_space()
            }

            Ok(JType::Object { v, l })
        } else {
            Err(Error::new(ErrorKind::Other, "Unknown type"))
        }
    }

    pub fn encode_rec(
        &self,
        cx: &mut FunctionContext<'a>,
        buf: &mut Vec<u8>,
        start: usize,
    ) -> Result<usize> {
        let length_varint = self.length() << TAG_SIZE | self.get_type();
        let varint = length_varint.encode_var_vec();
        let varint_length = varint.len();
        buf.write(&varint)?;

        Ok(varint_length
            + (match self {
                JType::Buffer { v } => buf.write(v),
                JType::String { v, l: _ } => buf.write(v.value(cx).as_bytes()),
                // JType::Int { v } => buf.write(&v.to_le_bytes()),
                JType::Number { v } => buf.write(&v.to_le_bytes()),
                // JType::Double { v: Right(float) } => buf.write(&float.to_le_bytes()),
                JType::BoolNull { v, l: _ } => match v {
                    None => Ok(JSON_NULL_SIZE),
                    Some(b) => buf.write(&[if *b { 1 } else { 0 }]),
                },
                JType::Array { v, l: _ } => {
                    let mut p = start;
                    for i in v {
                        p += i.encode_rec(cx, buf, p)?
                    }
                    Ok(p - start)
                }
                JType::Object { v, l: _ } => {
                    let mut p = start;
                    for (k, u) in v {
                        p += JType::String {
                            v: *k,
                            l: k.size(cx) as usize,
                        }
                        .encode_rec(cx, buf, p)?;
                        p += u.encode_rec(cx, buf, p)?;
                    }
                    Ok(p - start)
                }
            })?)
    }

    pub fn encode(&self, cx: &mut FunctionContext<'a>) -> Result<Vec<u8>> {
        let mut buf: Vec<u8> = Vec::with_capacity(self.length());
        self.encode_rec(cx, &mut buf, 0)?;

        buf.flush()?;

        Ok(buf)
    }

    pub fn length(&self) -> usize {
        match self {
            JType::Buffer { v } => v.len(),
            JType::String { v: _, l } => *l,
            // JType::Int { v: _ } => JSON_INT_SIZE,
            // JType::Double { v: _ } => JSON_DOUBLE_SIZE,
            JType::Number { v: _ } => JSON_DOUBLE_SIZE,
            JType::Array { v: _, l } => *l,
            JType::Object { v: _, l } => *l,
            JType::BoolNull { v: _, l } => *l,
        }
    }

    pub fn get_type(&self) -> usize {
        match self {
            JType::Buffer { v: _ } => BUFFER,
            JType::String { v: _, l: _ } => STRING,
            // JType::Int { v: _ } => INT,
            // JType::Double { v: _ } => DOUBLE,
            JType::Number { v: _ } => DOUBLE,
            JType::Array { v: _, l: _ } => ARRAY,
            JType::Object { v: _, l: _ } => OBJECT,
            JType::BoolNull { v: _, l: _ } => BOOLNULL,
        }
    }
}

pub fn decode_neon<'a>(mut cx: FunctionContext<'a>) -> JsResult<'a, JsValue> {
    let buf = cx.argument::<JsArrayBuffer>(0)?;
    let buf = cx.borrow(&buf, |x| x.as_slice::<u8>());

    let start = match cx.argument_opt(1) {
        Some(i) => match i.downcast::<JsNumber, _>(&mut cx) {
            Ok(i) => Ok(i.value(&mut cx) as usize),
            Err(_) => NeonResult::Err(neon::result::Throw),
        },
        None => Ok(0),
    }?;

    match decode_rec_neon(&mut cx, buf, start) {
        Ok(a) => Ok(a),
        Err(_) => NeonResult::Err(neon::result::Throw),
    }
}

pub fn decode_rec_neon<'a>(
    cx: &mut FunctionContext<'a>,
    buf: &[u8],
    start: usize,
) -> Result<Handle<'a, JsValue>> {
    let decoded: Option<(usize, usize)> = VarInt::decode_var(&buf[start..]);
    let (tag, bytes) = match decoded {
        None => Err(Error::from(ErrorKind::InvalidInput)),
        Some(v) => Result::Ok(v),
    }?;

    let field_type = tag & TAG_MASK;
    let len = tag >> TAG_SIZE;

    decode_type_neon(cx, field_type, buf, start + bytes, len)
}

pub fn decode_type_neon<'a>(
    cx: &mut FunctionContext<'a>,
    field_type: usize,
    buf: &[u8],
    start: usize,
    len: usize,
) -> Result<Handle<'a, JsValue>> {
    match field_type {
        STRING => Ok(decode_string_neon(cx, buf, start, len)?.upcast::<JsValue>()),
        BOOLNULL => Ok(decode_boolnull_neon(cx, buf, start, len)?.upcast::<JsValue>()),
        INT => Ok(decode_integer_neon(cx, buf, start)?.upcast::<JsValue>()),
        DOUBLE => Ok(decode_double_neon(cx, buf, start)?.upcast::<JsValue>()),
        ARRAY => Ok(decode_array_neon(cx, buf, start, len)?.upcast::<JsValue>()),
        OBJECT => Ok(decode_object_neon(cx, buf, start, len)?.upcast::<JsValue>()),
        BUFFER => Ok(decode_buffer_neon(cx, buf, start, len)?.upcast::<JsValue>()),
        _ => Err(Error::new(ErrorKind::Other, "invalid type")),
    }
}

pub fn decode_boolnull_neon<'a>(
    cx: &mut FunctionContext<'a>,
    buf: &[u8],
    start: usize,
    len: usize,
) -> Result<Handle<'a, JsValue>> {
    if len == 0 {
        Ok(JsNull::new(cx).upcast::<JsValue>())
    } else {
        let s = buf[start];
        if s > 2 {
            Err(Error::new(ErrorKind::Other, "Invalid boolnull"))
        } else {
            if len > 1 {
                Err(Error::new(
                    ErrorKind::Other,
                    "Invalid boolnull, len must be > 1",
                ))
            } else {
                Ok(JsBoolean::new(cx, if s == 1 { true } else { false }).upcast())
            }
        }
    }
}

pub fn decode_string_neon<'a>(
    cx: &mut FunctionContext<'a>,
    buf: &[u8],
    start: usize,
    len: usize,
) -> Result<Handle<'a, JsString>> {
    let raw_str = std::str::from_utf8(&buf[start..start + len]);
    match raw_str {
        std::result::Result::Ok(v) => Ok(JsString::new(cx, v)),
        std::result::Result::Err(_) => Err(Error::new(
            ErrorKind::Other,
            "Could not decode utf-8 string",
        )),
    }
}

pub fn decode_buffer_neon<'a>(
    cx: &mut FunctionContext<'a>,
    buf: &[u8],
    start: usize,
    len: usize,
) -> Result<Handle<'a, JsBuffer>> {
    let mut res = match JsBuffer::new(cx, len as u32) {
        Ok(b) => Ok(b),
        Err(_) => Err(Error::from(ErrorKind::InvalidInput)),
    }?;

    let mut out = cx.borrow_mut(&mut res, |x| x.as_mut_slice::<u8>());
    out.write(&buf[start..start + len])?;
    Ok(res)
}

pub fn decode_integer_neon<'a>(
    cx: &mut FunctionContext<'a>,
    buf: &[u8],
    start: usize,
) -> Result<Handle<'a, JsNumber>> {
    let bytes: [u8; 4] = buf[start..start + 4]
        .try_into()
        .expect("slice with incorrect length");
    Ok(JsNumber::new(cx, i32::from_le_bytes(bytes)))
}

pub fn decode_double_neon<'a>(
    cx: &mut FunctionContext<'a>,
    buf: &[u8],
    start: usize,
) -> Result<Handle<'a, JsNumber>> {
    let bytes: [u8; 8] = buf[start..start + 8]
        .try_into()
        .expect("slice with incorrect length");
    Ok(JsNumber::new(cx, f64::from_le_bytes(bytes)))
}

pub fn decode_array_neon<'a>(
    cx: &mut FunctionContext<'a>,
    buf: &[u8],
    start: usize,
    len: usize,
) -> Result<Handle<'a, JsArray>> {
    let mut c = 0;
    let mut vec: Vec<Handle<'a, JsValue>> = Vec::new();

    while c < len {
        let decoded: Option<(usize, usize)> = VarInt::decode_var(&buf[start + c..]);
        let (tag, bytes) = match decoded {
            None => Result::Err(Error::from(ErrorKind::InvalidInput)),
            Some(v) => Result::Ok(v),
        }?;

        c += bytes;

        let field_type = tag & TAG_MASK;
        let len = tag >> TAG_SIZE;

        vec.push(decode_type_neon(cx, field_type, buf, start + c, len)?);

        c += len;
    }

    let arr = JsArray::new(cx, vec.len() as u32);
    let mut i = 0;
    for x in vec {
        arr.set(cx, i, x);
        i += 1;
    }
    Ok(arr)
}

pub fn decode_object_neon<'a>(
    cx: &mut FunctionContext<'a>,
    buf: &[u8],
    start: usize,
    len: usize,
) -> Result<Handle<'a, JsObject>> {
    let mut c = 0;
    let obj = JsObject::new(&mut *cx);

    while c < len {
        let decoded: Option<(usize, usize)> = VarInt::decode_var(&buf[start + c..]);
        let (tag, bytes) = match decoded {
            None => Result::Err(Error::from(ErrorKind::InvalidInput)),
            Some(v) => Result::Ok(v),
        }?;
        c += bytes;
        let len = tag >> TAG_SIZE;
        let key = decode_string_neon(cx, buf, start + c, len)?;
        c += len;

        let decoded: Option<(usize, usize)> = VarInt::decode_var(&buf[start + c..]);
        let (tag, bytes) = match decoded {
            None => Err(Error::from(ErrorKind::InvalidInput)),
            Some(v) => Result::Ok(v),
        }?;

        let field_type = tag & TAG_MASK;
        let len = tag >> TAG_SIZE;

        c += bytes;
        let value = decode_type_neon(cx, field_type, buf, start + c, len)?;
        c += len;
        obj.set(cx, key, value);
    }

    Ok(obj)
}
