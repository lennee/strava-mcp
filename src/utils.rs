/// Format duration in seconds to "Xh Ym Zs" format
pub fn format_duration(seconds: u32) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, secs)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}

/// Format pace from meters per second to "min:sec/km" format
pub fn format_pace(meters_per_second: f64) -> String {
    if meters_per_second <= 0.0 {
        return "N/A".to_string();
    }

    // Convert to minutes per kilometer
    let seconds_per_km = 1000.0 / meters_per_second;
    let total_seconds = seconds_per_km.round() as u32;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;

    format!("{}:{:02}", minutes, seconds)
}

/// Format distance from meters to kilometers with 2 decimal places
pub fn format_distance(meters: f64) -> String {
    format!("{:.2}", meters / 1000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        // Test hours, minutes, and seconds
        assert_eq!(format_duration(3661), "1h 1m 1s");

        // Test only minutes and seconds
        assert_eq!(format_duration(125), "2m 5s");

        // Test only seconds
        assert_eq!(format_duration(45), "45s");

        // Test exact hour
        assert_eq!(format_duration(3600), "1h 0m 0s");

        // Test zero
        assert_eq!(format_duration(0), "0s");

        // Test large value
        assert_eq!(format_duration(7384), "2h 3m 4s");
    }

    #[test]
    fn test_format_pace() {
        // 4:00 min/km = 4.166... m/s
        // 1000m / 4.166 = 240 seconds = 4:00
        assert_eq!(format_pace(4.166666), "4:00");

        // 5:00 min/km = 3.333... m/s
        assert_eq!(format_pace(3.333333), "5:00");

        // 6:30 min/km = 2.564 m/s
        // 1000 / 2.564 = 390 seconds = 6:30
        assert_eq!(format_pace(2.564), "6:30");

        // Fast pace: 3:00 min/km = 5.555... m/s
        assert_eq!(format_pace(5.555556), "3:00");

        // Slow pace: 8:15 min/km = 2.020... m/s
        // 1000 / 2.020 = 495 seconds = 8:15
        assert_eq!(format_pace(2.020202), "8:15");

        // Edge case: zero or negative speed
        assert_eq!(format_pace(0.0), "N/A");
        assert_eq!(format_pace(-1.0), "N/A");
    }

    #[test]
    fn test_format_distance() {
        // 5 km
        assert_eq!(format_distance(5000.0), "5.00");

        // 10.5 km
        assert_eq!(format_distance(10500.0), "10.50");

        // Less than 1 km
        assert_eq!(format_distance(750.0), "0.75");

        // Zero
        assert_eq!(format_distance(0.0), "0.00");

        // Very long distance (marathon)
        assert_eq!(format_distance(42195.0), "42.20");

        // Exact kilometer
        assert_eq!(format_distance(1000.0), "1.00");
    }
}
