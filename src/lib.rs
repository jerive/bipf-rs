use neon::prelude::*;
mod bipf;
mod bindgen_impl;

pub use crate::bipf::*;
mod neon_impl;

#[neon::main]
fn main(mut cx: ModuleContext) -> NeonResult<()> {
    cx.export_function("encode", neon_impl::encode)?;
    cx.export_function("decode", neon_impl::decode)?;
    cx.export_function("encodingLength", neon_impl::encoding_length)?;
    cx.export_function("seekKey", neon_impl::seek_key)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::bipf::*;
    use serde_json::json;

    macro_rules! serde {
        ($($name:ident: $value:expr,)*) => {
        $(
            #[test]
            fn $name() {
                let input = $value;
                let buf = input.to_bipf();
                // println!("{:?}", buf.to_vec());

                let deserialized = decode(&buf.unwrap());

                assert_eq!(deserialized.is_ok(), true);
                assert_eq!(deserialized.unwrap().to_string(), input.to_string());
            }
        )*
        }
    }

    serde! {
        i100: json!(100),
        i0: json!(0),
        i1: json!(1),
        i_1: json!(-1),
        r#true: json!(true),
        r#false: json!(false),
        null: json!(null),
        empty_string: json!(""),
        empty_array: json!([]),
        empty_object: json!({}),
        array: json!([1,2,3,4,5,6,7,8,9]),
        string: json!("hello"),
        object: json!({ "foo": true}),
        complex: json!([-1, {"foo": true }]),
    }

    #[test]
    fn test_seek_key() {
        let bipf = json!({"hello": "unnecessary", "dependencies": { "rust": "v2.0.1" }})
            .to_bipf()
            .unwrap();
        let start = seek_key(&bipf, Some(0), String::from("dependencies"));

        assert_eq!(start.is_some(), true);

        assert_eq!(decode_rec(&bipf, start.unwrap()).unwrap().is_object(), true);
    }
}
