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
                let intermediary = JType::new(input.clone());
                let buf = intermediary.encode();
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
}
