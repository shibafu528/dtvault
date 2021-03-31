use std::path::{Component, PathBuf};

pub fn validate_file_name(s: &str) -> Result<(), String> {
    if s.is_empty() {
        return Err("Invalid value: file_name".to_string());
    }
    if s.contains("\0") {
        return Err("Invalid value: file_name".to_string());
    }
    for c in PathBuf::from(s).components() {
        match c {
            Component::Normal(_) => {}
            _ => return Err("Invalid value: file_name must only contain name".to_string()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_name_ascii() {
        assert_eq!(Ok(()), validate_file_name("test.mp4"));
    }

    #[test]
    fn test_file_name_jp() {
        assert_eq!(
            Ok(()),
            validate_file_name("ﾄﾞｷﾄﾞｷ!秘蔵のもふもふ動物動画大公開ＳＰ.mp4")
        );
    }

    #[test]
    fn test_file_name_empty() {
        assert_eq!(Err("Invalid value: file_name".to_string()), validate_file_name(""));
    }

    #[test]
    fn test_file_name_has_null_byte() {
        assert_eq!(
            Err("Invalid value: file_name".to_string()),
            validate_file_name("dt\0vault")
        );
    }

    #[test]
    fn test_file_name_relative_current_dir() {
        assert_eq!(
            Err("Invalid value: file_name must only contain name".to_string()),
            validate_file_name("./file.mp4")
        );
    }

    #[test]
    fn test_file_name_relative_parent_dir() {
        assert_eq!(
            Err("Invalid value: file_name must only contain name".to_string()),
            validate_file_name("../file.mp4")
        );
    }

    #[test]
    fn test_file_name_relative_illegal_dir() {
        assert_eq!(
            Err("Invalid value: file_name must only contain name".to_string()),
            validate_file_name("../../../.././../../../etc/passwd")
        );
    }
}
