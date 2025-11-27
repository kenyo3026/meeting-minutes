/// LRC (Lyric File) Parser Module
///
/// Parses LRC format files and converts them to transcript segments.
/// LRC format: [mm:ss.xx]text content
/// Example: [00:12.50]This is a transcript line

pub mod commands;

use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LrcLine {
    /// Start time in seconds from beginning
    pub time_seconds: f64,
    /// Text content of this line
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LrcParseResult {
    /// Parsed lines with timing information
    pub lines: Vec<LrcLine>,
    /// Any metadata found in the file (e.g., [ti:title], [ar:artist])
    pub metadata: LrcMetadata,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LrcMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub author: Option<String>,
    pub length: Option<String>,
}

/// Parse LRC file content into structured data
pub fn parse_lrc(content: &str) -> Result<LrcParseResult, String> {
    // Regex for LRC timestamp: [mm:ss.xx] or [mm:ss] or [mmm:ss.xx] (supports any number of minutes)
    let timestamp_regex = Regex::new(r"^\[(\d+):(\d{2})\.?(\d{0,2})\](.*)$")
        .map_err(|e| format!("Failed to compile regex: {}", e))?;

    let mut lines = Vec::new();
    let mut metadata = LrcMetadata::default();

    for line in content.lines() {
        let line = line.trim();

        // Skip empty lines
        if line.is_empty() {
            continue;
        }

        // Parse metadata tags
        if line.starts_with("[ti:") {
            metadata.title = extract_metadata_value(line);
            continue;
        }
        if line.starts_with("[ar:") {
            metadata.artist = extract_metadata_value(line);
            continue;
        }
        if line.starts_with("[al:") {
            metadata.album = extract_metadata_value(line);
            continue;
        }
        if line.starts_with("[au:") {
            metadata.author = extract_metadata_value(line);
            continue;
        }
        if line.starts_with("[length:") {
            metadata.length = extract_metadata_value(line);
            continue;
        }

        // Parse timestamp lines
        if let Some(captures) = timestamp_regex.captures(line) {
            let minutes: u32 = captures.get(1)
                .and_then(|m| m.as_str().parse().ok())
                .ok_or_else(|| "Invalid minutes in timestamp".to_string())?;

            let seconds: u32 = captures.get(2)
                .and_then(|m| m.as_str().parse().ok())
                .ok_or_else(|| "Invalid seconds in timestamp".to_string())?;

            // Centiseconds (hundredths of a second)
            let centiseconds_str = captures.get(3).map(|m| m.as_str()).unwrap_or("0");
            let centiseconds: u32 = if centiseconds_str.is_empty() {
                0
            } else {
                // Pad to 2 digits if only 1 digit provided
                let padded = if centiseconds_str.len() == 1 {
                    format!("{}0", centiseconds_str)
                } else {
                    centiseconds_str.to_string()
                };
                padded.parse().unwrap_or(0)
            };

            let text = captures.get(4)
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default();

            // Skip lines with empty text
            if text.is_empty() {
                continue;
            }

            // Convert to total seconds
            let time_seconds = (minutes * 60) as f64
                + seconds as f64
                + (centiseconds as f64 / 100.0);

            lines.push(LrcLine {
                time_seconds,
                text,
            });
        }
    }

    // Sort by time (in case lines are out of order)
    lines.sort_by(|a, b| a.time_seconds.partial_cmp(&b.time_seconds).unwrap());

    if lines.is_empty() {
        return Err("No valid LRC lines found in file".to_string());
    }

    Ok(LrcParseResult { lines, metadata })
}

/// Extract metadata value from LRC metadata tag
/// Example: "[ti:Song Title]" -> Some("Song Title")
fn extract_metadata_value(line: &str) -> Option<String> {
    line.find(':')
        .and_then(|pos| {
            let value = &line[pos + 1..];
            let value = value.trim_end_matches(']').trim();
            if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_lrc() {
        let content = r#"
[00:12.00]First line
[00:15.50]Second line
[00:20.30]Third line
        "#;

        let result = parse_lrc(content).unwrap();
        assert_eq!(result.lines.len(), 3);
        assert_eq!(result.lines[0].time_seconds, 12.0);
        assert_eq!(result.lines[0].text, "First line");
        assert_eq!(result.lines[1].time_seconds, 15.5);
        assert_eq!(result.lines[2].time_seconds, 20.3);
    }

    #[test]
    fn test_parse_with_metadata() {
        let content = r#"
[ti:Test Song]
[ar:Test Artist]
[al:Test Album]
[00:12.00]First line
        "#;

        let result = parse_lrc(content).unwrap();
        assert_eq!(result.metadata.title, Some("Test Song".to_string()));
        assert_eq!(result.metadata.artist, Some("Test Artist".to_string()));
        assert_eq!(result.metadata.album, Some("Test Album".to_string()));
        assert_eq!(result.lines.len(), 1);
    }

    #[test]
    fn test_parse_without_centiseconds() {
        let content = "[00:12]Line without centiseconds";
        let result = parse_lrc(content).unwrap();
        assert_eq!(result.lines[0].time_seconds, 12.0);
    }

    #[test]
    fn test_empty_file() {
        let content = "";
        let result = parse_lrc(content);
        assert!(result.is_err());
    }
}

