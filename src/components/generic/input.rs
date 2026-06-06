pub fn insert_char(s: &mut String, cursor: &mut usize, c: char) {
    let char_idx = *cursor;
    let char_count = s.chars().count();
    if char_idx >= char_count {
        s.push(c);
    } else if let Some((byte_idx, _)) = s.char_indices().nth(char_idx) {
        s.insert(byte_idx, c);
    }
    *cursor += 1;
}

pub fn delete_backwards(s: &mut String, cursor: &mut usize) {
    if *cursor == 0 {
        return;
    }
    *cursor -= 1;
    let char_idx = *cursor;
    if let Some((byte_idx, ch)) = s.char_indices().nth(char_idx) {
        s.drain(byte_idx..byte_idx + ch.len_utf8());
    }
}

pub fn delete_forwards(s: &mut String, cursor: usize) {
    let char_idx = cursor;
    if let Some((byte_idx, ch)) = s.char_indices().nth(char_idx) {
        s.drain(byte_idx..byte_idx + ch.len_utf8());
    }
}

pub fn move_word_left(s: &str, cursor: &mut usize) {
    if *cursor == 0 {
        return;
    }
    let chars: Vec<char> = s.chars().collect();
    let mut idx = *cursor;

    // Skip trailing spaces to the left
    while idx > 0 && chars[idx - 1].is_whitespace() {
        idx -= 1;
    }
    // Skip word characters
    while idx > 0 && !chars[idx - 1].is_whitespace() {
        idx -= 1;
    }
    *cursor = idx;
}

pub fn move_word_right(s: &str, cursor: &mut usize) {
    let chars: Vec<char> = s.chars().collect();
    let char_count = chars.len();
    if *cursor >= char_count {
        return;
    }
    let mut idx = *cursor;

    // Skip spaces to the right
    while idx < char_count && chars[idx].is_whitespace() {
        idx += 1;
    }
    // Skip word characters
    while idx < char_count && !chars[idx].is_whitespace() {
        idx += 1;
    }
    *cursor = idx;
}

pub fn delete_word_left(s: &mut String, cursor: &mut usize) {
    if *cursor == 0 {
        return;
    }
    let old_cursor = *cursor;
    move_word_left(s, cursor);
    let new_cursor = *cursor;

    let byte_start = s
        .char_indices()
        .nth(new_cursor)
        .map(|(b, _)| b)
        .unwrap_or(0);
    let byte_end = s
        .char_indices()
        .nth(old_cursor)
        .map(|(b, _)| b)
        .unwrap_or_else(|| s.len());
    s.drain(byte_start..byte_end);
}

pub fn delete_word_right(s: &mut String, cursor: usize) {
    let mut target_cursor = cursor;
    move_word_right(s, &mut target_cursor);

    let byte_start = s
        .char_indices()
        .nth(cursor)
        .map(|(b, _)| b)
        .unwrap_or_else(|| s.len());
    let byte_end = s
        .char_indices()
        .nth(target_cursor)
        .map(|(b, _)| b)
        .unwrap_or_else(|| s.len());
    s.drain(byte_start..byte_end);
}

pub fn delete_to_end(s: &mut String, cursor: usize) {
    let byte_idx = s
        .char_indices()
        .nth(cursor)
        .map(|(b, _)| b)
        .unwrap_or_else(|| s.len());
    s.drain(byte_idx..);
}

pub fn delete_to_start(s: &mut String, cursor: &mut usize) {
    let byte_idx = s
        .char_indices()
        .nth(*cursor)
        .map(|(b, _)| b)
        .unwrap_or_else(|| s.len());
    s.drain(..byte_idx);
    *cursor = 0;
}

pub fn move_left(cursor: &mut usize) {
    if *cursor > 0 {
        *cursor -= 1;
    }
}

pub fn move_right(s: &str, cursor: &mut usize) {
    let char_count = s.chars().count();
    if *cursor < char_count {
        *cursor += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_editing_helpers() {
        let mut s = String::from("hello world");
        let mut cursor = 5;

        // Insert char
        insert_char(&mut s, &mut cursor, '!');
        assert_eq!(s, "hello! world");
        assert_eq!(cursor, 6);

        // Delete backwards
        delete_backwards(&mut s, &mut cursor);
        assert_eq!(s, "hello world");
        assert_eq!(cursor, 5);

        // Move word left
        move_word_left(&s, &mut cursor);
        assert_eq!(cursor, 0); // start of "hello"

        // Move word right
        cursor = 0;
        move_word_right(&s, &mut cursor);
        assert_eq!(cursor, 5); // end of "hello"

        // Delete word left
        cursor = 11;
        delete_word_left(&mut s, &mut cursor);
        assert_eq!(s, "hello ");
        assert_eq!(cursor, 6);

        // Delete word right
        s = String::from("hello world");
        delete_word_right(&mut s, 5);
        assert_eq!(s, "hello");

        // Test delete_to_end
        s = String::from("hello world");
        delete_to_end(&mut s, 5);
        assert_eq!(s, "hello");

        // Test delete_to_start
        s = String::from("hello world");
        cursor = 6;
        delete_to_start(&mut s, &mut cursor);
        assert_eq!(s, "world");
        assert_eq!(cursor, 0);

        // Test move_left / move_right
        cursor = 2;
        move_left(&mut cursor);
        assert_eq!(cursor, 1);
        move_right("hello", &mut cursor);
        assert_eq!(cursor, 2);
    }
}
