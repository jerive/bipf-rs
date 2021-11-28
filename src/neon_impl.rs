use crate::bipf::*;
use integer_encoding::VarInt;
use neon::prelude::*;
use std::io::*;

const MAX_I32_F64: f64 = MAX_I32 as f64;

pub fn encoding_length<'a>(mut cx: FunctionContext<'a>) -> JsResult<JsNumber> {
    let arg = cx.argument::<JsValue>(0)?;
    match encoding_length_rec(&mut cx, arg) {
        Ok(l) => Ok(cx.number(l as f64)),
        Err(_) => NeonResult::Err(neon::result::Throw),
    }
}

pub fn encoding_length_rec<'a>(
    cx: &mut FunctionContext<'a>,
    input: Handle<'a, JsValue>,
) -> Result<usize> {
    let len = if input.is_a::<JsNull, _>(cx) {
        Ok(JSON_NULL_SIZE)
    } else if input.is_a::<JsBoolean, _>(cx) {
        Ok(JSON_BOOL_SIZE)
    } else if input.is_a::<JsString, _>(cx) {
        let res = match input.downcast::<JsString, _>(cx) {
            Ok(b) => Ok(b),
            _ => Err(Error::new(ErrorKind::Other, "")),
        }?;

        Ok(res.size(cx) as usize)
    } else if input.is_a::<JsNumber, _>(cx) {
        match input.downcast::<JsNumber, _>(cx) {
            Ok(x) => Ok({
                let v = x.value(cx);
                if v.abs() < MAX_I32_F64 && v.fract() == 0.0 {
                    JSON_INT_SIZE
                } else {
                    JSON_DOUBLE_SIZE
                }
            }),
            Err(_) => Err(Error::new(ErrorKind::Other, "")),
        }
    } else if input.is_a::<JsBuffer, _>(cx) {
        match input.downcast::<JsBuffer, _>(cx) {
            Ok(b) => Ok(cx.borrow(&b, |x| x.len())),
            Err(_) => Err(Error::new(ErrorKind::Other, "")),
        }
    } else if input.is_a::<JsArray, _>(cx) {
        match input.downcast::<JsArray, _>(cx) {
            Ok(res) => match res.to_vec(cx) {
                Ok(vec) => {
                    let mut l = 0;
                    for x in vec {
                        l += encoding_length_rec(cx, x)?
                    }

                    Ok(l)
                }
                Err(_) => Err(Error::new(ErrorKind::Other, "")),
            },
            Err(_) => Err(Error::new(ErrorKind::Other, "")),
        }
    } else if input.is_a::<JsObject, _>(cx) {
        let res = match input.downcast::<JsObject, _>(cx) {
            Ok(b) => Ok(b),
            _ => Err(Error::new(ErrorKind::Other, "")),
        }?;
        match res.get_own_property_names(cx).unwrap().to_vec(cx) {
            Ok(vec) => {
                let mut l = 0;
                for jskey in vec {
                    let key_len = encoding_length_rec(cx, jskey)?;
                    let obj = match res.get(cx, jskey) {
                        Ok(l) => Ok(l),
                        Err(_) => Err(Error::new(ErrorKind::Other, "")),
                    }?;
                    let val_length = encoding_length_rec(cx, obj)?;
                    l += key_len + val_length;
                }
                Ok(l)
            }
            Err(_) => Err(Error::new(ErrorKind::Other, "")),
        }
    } else {
        Err(Error::new(ErrorKind::Other, "Unknown type"))
    }?;

    Ok(len + (len << TAG_SIZE).required_space())
}

pub fn encode<'a>(mut cx: FunctionContext<'a>) -> JsResult<'a, JsBuffer> {
    let val = cx.argument::<JsValue>(0)?;
    match JType::new(val, &mut cx) {
        Ok(val) => match val.encode(&mut cx) {
            Ok(res) => {
                // Todo write continuously
                let mut buf = unsafe { JsBuffer::uninitialized(&mut cx, res.len() as u32) }?;
                let mut out = cx.borrow_mut(&mut buf, |x| x.as_mut_slice::<u8>());
                out.write(&res[..]);

                Ok(buf)
            }
            Err(_) => NeonResult::Err(neon::result::Throw),
        },
        Err(_) => NeonResult::Err(neon::result::Throw),
    }
}

pub fn seek_key<'a>(mut cx: FunctionContext<'a>) -> JsResult<'a, JsNumber> {
    let buf = cx.argument::<JsBuffer>(0)?;
    let bytes = cx.borrow(&buf, |x| x.as_slice());
    let start = cx.argument::<JsNumber>(1)?.value(&mut cx) as isize;
    let start = if start == -1 {
        None
    } else {
        Some(start as usize)
    };
    let tmp_string: String;
    let tmp_buf: Handle<JsBuffer>;
    let target = cx.argument::<JsValue>(2)?;
    let target: &[u8] = if target.is_a::<JsBuffer, _>(&mut cx) {
        tmp_buf = target.downcast_or_throw::<JsBuffer, _>(&mut cx)?;
        cx.borrow(&tmp_buf, |x| x.as_slice::<u8>())
    } else if target.is_a::<JsString, _>(&mut cx) {
        let f = target.downcast_or_throw::<JsString, _>(&mut cx)?;
        tmp_string = f.value(&mut cx);
        tmp_string.as_bytes()
    } else {
        return cx.throw_error("expected 3rd argument to `seek_key` to be a string or buffer");
    };

    Ok(cx.number(match seek_key_internal(bytes, start, &target) {
        None => -1 as f64,
        Some(v) => v as f64,
    }))
}

fn seek_key_internal<'a>(bytes: &[u8], start: Option<usize>, target: &[u8]) -> Option<usize> {
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
                let target_buf = target;
                while c < len {
                    let key_tag: (usize, usize) = VarInt::decode_var(&bytes[start + c..])?;
                    c += key_tag.1;
                    let key_len = key_tag.0 >> TAG_SIZE;
                    let key_type = key_tag.0 & TAG_MASK;

                    if key_type == STRING && target_length == key_len {
                        if target_buf == &bytes[start + c..start + c + target_length] {
                            let next_start = start + c + key_len;
                            return Some(next_start);
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

enum JType<'a> {
    String {
        v: Handle<'a, JsString>,
        l: usize,
    },
    Buffer {
        v: Vec<u8>,
    },
    Int {
        v: i32,
    },
    Double {
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
            Ok(match input.downcast::<JsNumber, _>(cx) {
                Ok(x) => Ok({
                    let v = x.value(cx);
                    // TODO properly handle numbers
                    // https://medium.com/angular-in-depth/javascripts-number-type-8d59199db1b6#.9whwe88tz
                    // https://stackoverflow.com/questions/48500261/check-if-a-float-can-be-converted-to-integer-without-loss/48500414
                    // Also cf https://github.com/ssbc/bipf/issues/2
                    if v.abs() < MAX_I32_F64 && v.fract() == 0.0 {
                        JType::Int { v: v as i32 }
                    } else {
                        JType::Double { v }
                    }
                }),
                Err(_) => Err(Error::new(ErrorKind::Other, "")),
            }?)
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
                    Err(_) => Err(Error::new(ErrorKind::Other, "")),
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
                JType::Int { v } => buf.write(&v.to_le_bytes()),
                JType::Double { v } => buf.write(&v.to_le_bytes()),
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
            JType::Int { v: _ } => JSON_INT_SIZE,
            JType::Double { v: _ } => JSON_DOUBLE_SIZE,
            JType::Array { v: _, l } => *l,
            JType::Object { v: _, l } => *l,
            JType::BoolNull { v: _, l } => *l,
        }
    }

    pub fn get_type(&self) -> usize {
        match self {
            JType::Buffer { v: _ } => BUFFER,
            JType::String { v: _, l: _ } => STRING,
            JType::Int { v: _ } => INT,
            JType::Double { v: _ } => DOUBLE,
            JType::Array { v: _, l: _ } => ARRAY,
            JType::Object { v: _, l: _ } => OBJECT,
            JType::BoolNull { v: _, l: _ } => BOOLNULL,
        }
    }
}

pub fn decode<'a>(mut cx: FunctionContext<'a>) -> JsResult<'a, JsValue> {
    let buf = cx.argument::<JsBuffer>(0)?;
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
        Some(v) => Ok(v),
        None => Err(Error::from(ErrorKind::InvalidInput)),
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
        STRING => decode_string_neon(cx, buf, start, len),
        BOOLNULL => decode_boolnull_neon(cx, buf, start, len),
        INT => decode_integer_neon(cx, buf, start),
        DOUBLE => decode_double_neon(cx, buf, start),
        ARRAY => decode_array_neon(cx, buf, start, len),
        OBJECT => decode_object_neon(cx, buf, start, len),
        BUFFER => decode_buffer_neon(cx, buf, start, len),
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
        Ok(cx.null().upcast())
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
                Ok(cx.boolean(if s == 1 { true } else { false }).upcast())
            }
        }
    }
}

pub fn decode_string_neon<'a>(
    cx: &mut FunctionContext<'a>,
    buf: &[u8],
    start: usize,
    len: usize,
) -> Result<Handle<'a, JsValue>> {
    let raw_str = std::str::from_utf8(&buf[start..start + len]);
    match raw_str {
        std::result::Result::Ok(v) => Ok(cx.string(v).upcast()),
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
) -> Result<Handle<'a, JsValue>> {
    let mut res = match cx.buffer(len as u32) {
        Ok(b) => Ok(b),
        Err(_) => Err(Error::from(ErrorKind::InvalidInput)),
    }?;

    let mut out = cx.borrow_mut(&mut res, |x| x.as_mut_slice::<u8>());
    out.write(&buf[start..start + len])?;
    Ok(res.upcast())
}

pub fn decode_integer_neon<'a>(
    cx: &mut FunctionContext<'a>,
    buf: &[u8],
    start: usize,
) -> Result<Handle<'a, JsValue>> {
    let bytes: [u8; 4] = buf[start..start + 4]
        .try_into()
        .expect("slice with incorrect length");
    Ok(cx.number(i32::from_le_bytes(bytes)).upcast())
}

pub fn decode_double_neon<'a>(
    cx: &mut FunctionContext<'a>,
    buf: &[u8],
    start: usize,
) -> Result<Handle<'a, JsValue>> {
    let bytes: [u8; 8] = buf[start..start + 8]
        .try_into()
        .expect("slice with incorrect length");
    Ok(cx.number(f64::from_le_bytes(bytes)).upcast())
}

pub fn decode_array_neon<'a>(
    cx: &mut FunctionContext<'a>,
    buf: &[u8],
    start: usize,
    len: usize,
) -> Result<Handle<'a, JsValue>> {
    let mut c = 0;
    let mut vec: Vec<Handle<'a, JsValue>> = Vec::new();

    while c < len {
        let decoded: Option<(usize, usize)> = VarInt::decode_var(&buf[start + c..]);
        let (tag, bytes) = match decoded {
            Some(v) => Result::Ok(v),
            None => Result::Err(Error::from(ErrorKind::InvalidInput)),
        }?;

        c += bytes;

        let field_type = tag & TAG_MASK;
        let len = tag >> TAG_SIZE;

        vec.push(decode_type_neon(cx, field_type, buf, start + c, len)?);

        c += len;
    }

    let arr = cx.empty_array();
    let mut i = 0;
    for x in vec {
        arr.set(cx, i, x);
        i += 1;
    }
    Ok(arr.upcast())
}

pub fn decode_object_neon<'a>(
    cx: &mut FunctionContext<'a>,
    buf: &[u8],
    start: usize,
    len: usize,
) -> Result<Handle<'a, JsValue>> {
    let mut c = 0;
    let obj = cx.empty_object();

    while c < len {
        let decoded: Option<(usize, usize)> = VarInt::decode_var(&buf[start + c..]);
        let (tag, bytes) = match decoded {
            Some(v) => Result::Ok(v),
            None => Result::Err(Error::from(ErrorKind::InvalidInput)),
        }?;
        c += bytes;
        let len = tag >> TAG_SIZE;
        let key = decode_string_neon(cx, buf, start + c, len)?;
        c += len;

        let decoded: Option<(usize, usize)> = VarInt::decode_var(&buf[start + c..]);
        let (tag, bytes) = match decoded {
            Some(v) => Result::Ok(v),
            None => Err(Error::from(ErrorKind::InvalidInput)),
        }?;

        let field_type = tag & TAG_MASK;
        let len = tag >> TAG_SIZE;

        c += bytes;
        let value = decode_type_neon(cx, field_type, buf, start + c, len)?;
        c += len;
        obj.set(cx, key, value);
    }

    Ok(obj.upcast())
}
