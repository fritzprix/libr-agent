use serde::{Deserialize, Serialize};

/// Helper to serialize optional JSON fields to Option<String>.
/// Returns None if the input is None.
/// Returns Ok(Some(json_string)) if serialization succeeds.
/// Returns Err if serialization fails.
pub fn to_json_option<T: Serialize>(value: &Option<T>) -> Result<Option<String>, serde_json::Error> {
    value.as_ref().map(serde_json::to_string).transpose()
}

/// Helper to deserialize optional JSON string fields to Option<T>.
/// Returns None if the input is None or if deserialization fails (silently ignoring errors).
/// Use this when you want to handle potential schema evolution gracefully.
pub fn from_json_option<T: for<'a> Deserialize<'a>>(value: &Option<String>) -> Option<T> {
    value.as_ref().and_then(|s| serde_json::from_str(s).ok())
}

/// Helper to deserialize a JSON string to T, or return T::default() if deserialization fails.
/// Useful for mandatory fields that might be corrupt or in an old format.
pub fn from_json_or_default<T: for<'a> Deserialize<'a> + Default>(value: &str) -> T {
    serde_json::from_str(value).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_json_option() {
        let some_val = Some(vec![1, 2, 3]);
        let none_val: Option<Vec<i32>> = None;

        assert_eq!(
            to_json_option(&some_val).unwrap(),
            Some("[1,2,3]".to_string())
        );
        assert_eq!(to_json_option(&none_val).unwrap(), None);
    }

    #[test]
    fn test_from_json_option() {
        let json_str = Some("[1,2,3]".to_string());
        let none_str: Option<String> = None;
        let bad_json = Some("invalid".to_string());

        let res: Option<Vec<i32>> = from_json_option(&json_str);
        assert_eq!(res, Some(vec![1, 2, 3]));

        let res: Option<Vec<i32>> = from_json_option(&none_str);
        assert_eq!(res, None);

        let res: Option<Vec<i32>> = from_json_option(&bad_json);
        assert_eq!(res, None);
    }

    #[test]
    fn test_from_json_or_default() {
        let json_str = "[1,2,3]";
        let bad_json = "invalid";

        let res: Vec<i32> = from_json_or_default(json_str);
        assert_eq!(res, vec![1, 2, 3]);

        let res: Vec<i32> = from_json_or_default(bad_json);
        assert_eq!(res, Vec::<i32>::new());
    }
}
