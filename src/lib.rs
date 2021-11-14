pub mod bipf;

#[cfg(test)]
mod tests {
    use crate::bipf::*;
    use serde_json::json;

    #[test]
    fn it_does_intermediate() {
        let n = JType::new(json!({ "test": 100 }));
        match n {
            JType::Object { v: _, l } => {
                assert_eq!(l, 10);
            }
            _ => panic!("Failure")
        }
    }

    #[test]
    fn it_serializes() {
        let n = JType::new(json!([{"": 1000}]));
        let buf = n.encode();
        println!("{:?}", buf.to_vec());

        let deser = decode(&buf);

        assert_eq!(deser.is_ok(), true);
        println!("{:?}", deser.unwrap());
    }
}
