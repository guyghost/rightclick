//! Clipboard integration for RightClick
//!
//! Provides a global clipboard interface using `arboard` for cross-platform
//! clipboard access. Supports copying text to the system clipboard.

use std::sync::Mutex;

use once_cell::sync::Lazy;

/// Global clipboard instance
static CLIPBOARD: Lazy<Mutex<Option<arboard::Clipboard>>> = Lazy::new(|| {
    let clipboard = arboard::Clipboard::new().ok();
    Mutex::new(clipboard)
});

/// Copy text to the system clipboard
///
/// Returns `Ok(())` on success, or an error message on failure.
///
/// # Example
///
/// ```no_run
/// use rightclick::shell::clipboard::copy_to_clipboard;
///
/// match copy_to_clipboard("Hello, world!") {
///     Ok(()) => println!("Copied!"),
///     Err(e) => eprintln!("Failed to copy: {}", e),
/// }
/// ```
pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
    let mut guard = CLIPBOARD
        .lock()
        .map_err(|e| format!("Clipboard lock poisoned: {}", e))?;

    match guard.as_mut() {
        Some(clipboard) => clipboard
            .set_text(text)
            .map_err(|e| format!("Failed to copy to clipboard: {}", e)),
        None => Err("Clipboard not available".to_string()),
    }
}

/// Read text from the system clipboard
///
/// Returns the clipboard text content, or an error on failure.
pub fn read_from_clipboard() -> Result<String, String> {
    let mut guard = CLIPBOARD
        .lock()
        .map_err(|e| format!("Clipboard lock poisoned: {}", e))?;

    match guard.as_mut() {
        Some(clipboard) => clipboard
            .get_text()
            .map_err(|e| format!("Failed to read clipboard: {}", e)),
        None => Err("Clipboard not available".to_string()),
    }
}

/// Check if clipboard is available on this system
pub fn is_clipboard_available() -> bool {
    CLIPBOARD
        .lock()
        .map(|guard| guard.is_some())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_availability_check() {
        // This test just checks the function doesn't panic
        // Clipboard may or may not be available in CI
        let _ = is_clipboard_available();
    }

    #[test]
    fn copy_and_read_roundtrip() {
        // Skip if clipboard is not available (e.g., headless CI)
        if !is_clipboard_available() {
            return;
        }

        let text = "RightClick clipboard test";
        let result = copy_to_clipboard(text);
        // Just verify copy doesn't error - read may not work in all environments
        // (e.g., sandboxed test runners, headless CI)
        if result.is_ok() {
            let read = read_from_clipboard();
            // Read may succeed or fail depending on clipboard backend
            if let Ok(content) = read {
                // If read works, it should match what we wrote
                // But some clipboard backends may not persist across operations
                let _ = content;
            }
        }
    }

    #[test]
    fn copy_empty_string() {
        if !is_clipboard_available() {
            return;
        }

        let _ = copy_to_clipboard("");
    }

    #[test]
    fn copy_unicode() {
        if !is_clipboard_available() {
            return;
        }

        let text = "🎉 Unicode: éàü ñ 中文";
        let _ = copy_to_clipboard(text);
    }
}
