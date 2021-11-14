pub mod bipf;

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

                let deserialized = decode(&buf);

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
        let bipf = json!({"hello": "unnecessary", "dependencies": { "rust": "v2.0.1" }}).to_bipf();
        let cloned = bipf.clone();
        let start = seek_key(bipf, Some(0), String::from("dependencies"));

        assert_eq!(start.is_some(), true);

        assert_eq!(decode_rec(&cloned, start.unwrap()).unwrap().is_object(), true);
    }
}
