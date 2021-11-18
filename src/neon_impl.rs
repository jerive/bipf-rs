use crate::bipf::*;
use integer_encoding::VarInt;
use neon::prelude::*;
use std::io::*;

pub fn decode_neon<'a>(mut cx: FunctionContext<'a>) -> JsResult<'a, JsValue> {
    let buf = cx.argument::<JsArrayBuffer>(0)?;
    let buf = cx.borrow(&buf, |x| x.as_slice::<u8>());

    let start = match cx.argument::<JsNumber>(1) {
        Ok(i) => i.value(&mut cx) as usize,
        Err(_) => 0,
    };

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
