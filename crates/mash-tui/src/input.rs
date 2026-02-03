//! Form input handling for the TUI.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Input mode for text fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Editing,
}

/// A text input field with cursor support.
#[derive(Debug, Clone)]
pub struct InputField {
    /// Current input value.
    value: String,
    /// Cursor position (byte index).
    cursor: usize,
    /// Placeholder/label text.
    pub placeholder: String,
    /// Current input mode.
    pub mode: InputMode,
}

impl InputField {
    pub fn new(default: String, placeholder: &str) -> Self {
        let cursor = default.len();
        Self {
            value: default,
            cursor,
            placeholder: placeholder.to_string(),
            mode: InputMode::Normal,
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn set_value(&mut self, value: String) {
        self.cursor = value.len();
        self.value = value;
    }

    /// Handle a key event, returns true if the event was consumed.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char(c) => {
                // Handle Ctrl+A (select all / move to start).
                if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'a' {
                    self.cursor = 0;
                    return true;
                }
                // Handle Ctrl+E (move to end).
                if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'e' {
                    self.cursor = self.value.len();
                    return true;
                }
                // Handle Ctrl+U (clear line).
                if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'u' {
                    self.value.clear();
                    self.cursor = 0;
                    return true;
                }
                // Handle Ctrl+K (kill to end of line).
                if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'k' {
                    self.value.truncate(self.cursor);
                    return true;
                }
                // Handle Ctrl+W (delete word backward).
                if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'w' {
                    self.delete_word_backward();
                    return true;
                }
                // Normal character input.
                self.insert_char(c);
                true
            }
            KeyCode::Backspace => {
                self.delete_char_backward();
                true
            }
            KeyCode::Delete => {
                self.delete_char_forward();
                true
            }
            KeyCode::Left => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.move_word_backward();
                } else {
                    self.move_cursor_left();
                }
                true
            }
            KeyCode::Right => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.move_word_forward();
                } else {
                    self.move_cursor_right();
                }
                true
            }
            KeyCode::Home => {
                self.cursor = 0;
                true
            }
            KeyCode::End => {
                self.cursor = self.value.len();
                true
            }
            _ => false,
        }
    }

    fn insert_char(&mut self, c: char) {
        self.value.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    fn delete_char_backward(&mut self) {
        if self.cursor > 0 {
            // Find the previous character boundary.
            let prev = self.value[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.value.remove(prev);
            self.cursor = prev;
        }
    }

    fn delete_char_forward(&mut self) {
        if self.cursor < self.value.len() {
            self.value.remove(self.cursor);
        }
    }

    fn move_cursor_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.value[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    fn move_cursor_right(&mut self) {
        if self.cursor < self.value.len() {
            self.cursor = self.value[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.value.len());
        }
    }

    fn move_word_backward(&mut self) {
        // Skip whitespace, then skip word characters.
        let chars: Vec<_> = self.value[..self.cursor].char_indices().collect();

        // Skip trailing whitespace.
        let mut i = chars.len();
        while i > 0 && chars[i - 1].1.is_whitespace() {
            i -= 1;
        }
        // Skip word characters.
        while i > 0 && !chars[i - 1].1.is_whitespace() {
            i -= 1;
        }

        if i > 0 {
            self.cursor = chars[i].0;
        } else {
            self.cursor = 0;
        }
    }

    fn move_word_forward(&mut self) {
        let chars: Vec<_> = self.value[self.cursor..].char_indices().collect();
        if chars.is_empty() {
            return;
        }

        let mut i = 0;
        // Skip word characters.
        while i < chars.len() && !chars[i].1.is_whitespace() {
            i += 1;
        }
        // Skip whitespace.
        while i < chars.len() && chars[i].1.is_whitespace() {
            i += 1;
        }

        if i < chars.len() {
            self.cursor += chars[i].0;
        } else {
            self.cursor = self.value.len();
        }
    }

    fn delete_word_backward(&mut self) {
        let old_cursor = self.cursor;
        self.move_word_backward();
        let new_cursor = self.cursor;
        self.value.drain(new_cursor..old_cursor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_field_basic() {
        let mut field = InputField::new("hello".to_string(), "test");
        assert_eq!(field.value(), "hello");
        assert_eq!(field.cursor(), 5);

        field.insert_char('!');
        assert_eq!(field.value(), "hello!");
        assert_eq!(field.cursor(), 6);

        field.delete_char_backward();
        assert_eq!(field.value(), "hello");
        assert_eq!(field.cursor(), 5);
    }

    #[test]
    fn test_cursor_movement() {
        let mut field = InputField::new("hello world".to_string(), "test");
        field.cursor = 5;

        field.move_cursor_left();
        assert_eq!(field.cursor(), 4);

        field.move_cursor_right();
        assert_eq!(field.cursor(), 5);

        field.cursor = 0;
        field.move_cursor_left();
        assert_eq!(field.cursor(), 0); // Should stay at 0.
    }
}
