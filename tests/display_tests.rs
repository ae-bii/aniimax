//! Tests for display and formatting utilities.

use aniimax::display::format_time;

#[test]
fn test_format_time_seconds() {
    assert_eq!(format_time(30.0), "30s");
    assert_eq!(format_time(59.0), "59s");
}

#[test]
fn test_format_time_minutes() {
    assert_eq!(format_time(60.0), "1m 0s");
    assert_eq!(format_time(90.0), "1m 30s");
    assert_eq!(format_time(300.0), "5m 0s");
}

#[test]
fn test_format_time_hours() {
    assert_eq!(format_time(3600.0), "1h 0m 0s");
    assert_eq!(format_time(3661.0), "1h 1m 1s");
    assert_eq!(format_time(7200.0), "2h 0m 0s");
}

#[test]
fn test_format_time_zero() {
    assert_eq!(format_time(0.0), "0s");
}

#[test]
fn test_format_time_fractional() {
    // Should handle fractional seconds by truncating
    assert_eq!(format_time(30.5), "30s");
    assert_eq!(format_time(90.9), "1m 31s"); // 90.9 seconds = 1m 30.9s, rounds to 1m 31s
}
