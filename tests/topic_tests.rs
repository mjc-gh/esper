extern crate esper;

#[cfg(test)]
mod tests {
    use esper::Topic;

    #[test]
    fn valid_alphanumeric_topic_id() {
        let topic_opt = Topic::validate(0, &"abcdef123".to_owned());

        assert!(topic_opt.is_some());
    }

    #[test]
    fn invalid_to_short_topic_id() {
        let topic_opt = Topic::validate(0, &"abc".to_owned());

        assert!(topic_opt.is_none());
    }

    #[test]
    fn invalid_to_long_topic_id() {
        let topic_opt = Topic::validate(0, &"abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdef".to_owned());

        assert!(topic_opt.is_none());
    }

    #[test]
    fn invalid_non_alphanumeric_topic_id() {
        let topic_opt = Topic::validate(0, &"abcdef123\0".to_owned());

        assert!(topic_opt.is_none());
    }

    #[test]
    fn topic_validate_does_skip() {
        let topic_opt = Topic::validate(5, &"/xxx/abcdef123".to_owned());

        assert!(topic_opt.is_some());
    }
}
