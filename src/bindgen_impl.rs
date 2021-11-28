use crate::bipf::*;

use node_bindgen::derive::node_bindgen;
use node_bindgen::sys::*;
use node_bindgen::core::NjError;
use node_bindgen::core::TryIntoJs;
use node_bindgen::core::buffer::*;
use node_bindgen::core::val::*;
use node_bindgen::core::buffer::ArrayBuffer;
use integer_encoding::*;

#[node_bindgen(name="decode")]
fn bindgen_decode(value: JSArrayBuffer, start: f64, env: JsEnv) -> Result<napi_value, NjError> {
    let start = start as usize;
    decode_rec_bindgen(env, &value, start as usize)
}

pub fn decode_rec_bindgen(
    env: JsEnv,
    buf: &[u8],
    start: usize,
) -> Result<napi_value, NjError> {
    let decoded: Option<(usize, usize)> = VarInt::decode_var(&buf[start..]);
    let (tag, bytes) = match decoded {
        Some(v) => Ok(v),
        None => Err(NjError::Other(String::from(""))),
    }?;

    let field_type = tag & TAG_MASK;
    let len = tag >> TAG_SIZE;

    decode_type_bindgen(env, field_type, buf, start + bytes, len)
}

pub fn decode_type_bindgen(
    cx: JsEnv,
    field_type: usize,
    buf: &[u8],
    start: usize,
    len: usize,
) -> Result<napi_value, NjError> {
    match field_type {
        STRING => decode_string_bindgen(cx, buf, start, len),
        BOOLNULL => decode_boolnull_bindgen(cx, buf, start, len),
        INT => decode_integer_bindgen(cx, buf, start),
        DOUBLE => decode_double_bindgen(cx, buf, start),
        ARRAY => decode_array_bindgen(cx, buf, start, len),
        OBJECT => decode_object_bindgen(cx, buf, start, len),
        BUFFER => Ok(decode_buffer_bindgen(cx, buf, start, len)?),
        _ => Err(NjError::Other(String::from(""))),
    }
}

pub fn decode_boolnull_bindgen(
    cx: JsEnv,
    buf: &[u8],
    start: usize,
    len: usize,
) -> Result<napi_value, NjError> {
    if len == 0 {
        cx.get_null()
    } else {
        let s = buf[start];
        if s > 2 {
            Err(NjError::Other(String::from("Invalid boolnull")))
        } else {
            if len > 1 {
                Err(NjError::Other(String::from(
                    "Invalid boolnull, len must be > 1",
                )))
            } else {
                cx.create_boolean(if s == 1 { true } else { false })
            }
        }
    }
}

pub fn decode_string_bindgen(
    cx: JsEnv,
    buf: &[u8],
    start: usize,
    len: usize,
) -> Result<napi_value, NjError> {
    cx.create_string_utf8_from_bytes(&buf[start..start + len])
}

pub fn decode_buffer_bindgen(
    cx: JsEnv,
    buf: &[u8],
    start: usize,
    len: usize,
) -> Result<napi_value, NjError>{
    ArrayBuffer::new(buf[start..start + len].to_vec()).try_to_js(&cx)
}

pub fn decode_integer_bindgen(
    cx: JsEnv,
    buf: &[u8],
    start: usize,
) -> Result<napi_value, NjError>{
    let bytes: [u8; 4] = buf[start..start + 4]
        .try_into()
        .expect("slice with incorrect length");
    cx.create_double(i32::from_le_bytes(bytes)as f64)
}

pub fn decode_double_bindgen(
    cx: JsEnv,
    buf: &[u8],
    start: usize,
) -> Result<napi_value, NjError>{
    let bytes: [u8; 8] = buf[start..start + 8]
        .try_into()
        .expect("slice with incorrect length");
    cx.create_double(f64::from_le_bytes(bytes))
}

pub fn decode_array_bindgen(
    cx: JsEnv,
    buf: &[u8],
    start: usize,
    len: usize,
) -> Result<napi_value, NjError>{
    let mut c = 0;
    let arr = cx.create_array_with_len(0)?;
    let mut i = 0;
    while c < len {
        let decoded: Option<(usize, usize)> = VarInt::decode_var(&buf[start + c..]);
        let (tag, bytes) = match decoded {
            Some(v) => Result::Ok(v),
            None => Err(NjError::Other(String::from(
                "Could not decode varint"
            ))),
        }?;

        c += bytes;

        let field_type = tag & TAG_MASK;
        let len = tag >> TAG_SIZE;

        cx.set_element(arr, decode_type_bindgen(cx, field_type, buf, start + c, len)?, i)?;
        i += 1;
        c += len;
    }

    Ok(arr)
}

pub fn decode_object_bindgen(
    cx: JsEnv,
    buf: &[u8],
    start: usize,
    len: usize,
) -> Result<napi_value, NjError>{
    let mut c = 0;
    let obj = cx.create_object()?;
    let napi_env = cx.inner();

    while c < len {
        let decoded: Option<(usize, usize)> = VarInt::decode_var(&buf[start + c..]);
        let (tag, bytes) = match decoded {
            Some(v) => Result::Ok(v),
            None => Err(NjError::Other(String::from(
                "Could not decode varint"
            ))),
        }?;
        c += bytes;
        let len = tag >> TAG_SIZE;
        let key = match std::ffi::CString::new(&buf[start+c ..start +c+ len]) {
            Ok(s) => Ok(s),
            Err(_) => Err(NjError::Other(String::from(
                "Could not create string"
            ))),
        }?;
        c += len;

        let decoded: Option<(usize, usize)> = VarInt::decode_var(&buf[start + c..]);
        let (tag, bytes) = match decoded {
            Some(v) => Result::Ok(v),
            None => Err(NjError::Other(String::from(
                "Could not decode varint"
            ))),
        }?;

        let field_type = tag & TAG_MASK;
        let len = tag >> TAG_SIZE;

        c += bytes;
        let value = decode_type_bindgen(cx, field_type, buf, start + c, len)?;
        c += len;
        
        unsafe {
            napi_set_named_property(napi_env, obj, key.as_ptr(), value);
        }        
    }

    Ok(obj)
}
