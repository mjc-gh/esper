extern crate esper;

#[cfg(test)]
mod tests {
    use esper::Message;

    #[test]
    fn new_message_appends_delimiter() {
        let msg = Message::new(&vec![65 as u8]);
        let expected = vec![65 as u8, 10 as u8, 10 as u8];

        assert_eq!(expected.as_slice(), msg.as_slice());
    }
}
