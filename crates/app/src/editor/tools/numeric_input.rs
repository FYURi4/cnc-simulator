use winit::keyboard::KeyCode;

/// Обработка клавиш числового ввода.
/// Стрелка вправо автоматически вставляет десятичную точку.
pub fn handle_key(buffer: &mut String, key: KeyCode) -> bool {
    let digit = match key {
        KeyCode::Digit0 | KeyCode::Numpad0 => Some('0'),
        KeyCode::Digit1 | KeyCode::Numpad1 => Some('1'),
        KeyCode::Digit2 | KeyCode::Numpad2 => Some('2'),
        KeyCode::Digit3 | KeyCode::Numpad3 => Some('3'),
        KeyCode::Digit4 | KeyCode::Numpad4 => Some('4'),
        KeyCode::Digit5 | KeyCode::Numpad5 => Some('5'),
        KeyCode::Digit6 | KeyCode::Numpad6 => Some('6'),
        KeyCode::Digit7 | KeyCode::Numpad7 => Some('7'),
        KeyCode::Digit8 | KeyCode::Numpad8 => Some('8'),
        KeyCode::Digit9 | KeyCode::Numpad9 => Some('9'),
        _ => None,
    };

    if let Some(digit) = digit {
        buffer.push(digit);
        return true;
    }

    match key {
        // Автоматическая десятичная точка.
        KeyCode::ArrowRight | KeyCode::Period | KeyCode::Comma | KeyCode::NumpadDecimal => {
            if !buffer.contains('.') {
                if buffer.is_empty() || buffer == "-" {
                    buffer.push_str("0.");
                } else {
                    buffer.push('.');
                }
            }
            true
        }

        // Изменение знака.
        KeyCode::Minus | KeyCode::NumpadSubtract => {
            if buffer.starts_with('-') {
                buffer.remove(0);
            } else {
                buffer.insert(0, '-');
            }
            true
        }

        // Удаление последней цифры.
        KeyCode::Backspace => {
            buffer.pop();
            true
        }

        // Полная очистка.
        KeyCode::Delete => {
            buffer.clear();
            true
        }

        _ => false,
    }
}

/// Преобразование строки в конечное число.
pub fn parse(buffer: &str) -> Option<f32> {
    if buffer.is_empty() || buffer == "-" || buffer == "." || buffer == "-." {
        return None;
    }

    let normalized = if buffer.ends_with('.') {
        format!("{}0", buffer)
    } else {
        buffer.to_string()
    };

    let value = normalized.parse::<f32>().ok()?;

    if value.is_finite() {
        Some(value)
    } else {
        None
    }
}
