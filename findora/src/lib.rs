//! findora
//!
//! This module implements a variety of tools for
//! general use.

use std::sync::atomic::AtomicI8;
use std::sync::atomic::Ordering;

/// This structure provides the enable flags for logging.
#[derive(Default)]
pub struct EnableMap {
  pub error_enabled: AtomicI8,
  pub debug_enabled: AtomicI8,
  pub warning_enabled: AtomicI8,
  pub info_enabled: AtomicI8,
  pub log_enabled: AtomicI8,
}

// Define a set of defaults for anyone who
// prefers one.
pub const DEFAULT_MAP: EnableMap = EnableMap { error_enabled: AtomicI8::new(1),
                                               debug_enabled: AtomicI8::new(0),
                                               warning_enabled: AtomicI8::new(1),
                                               info_enabled: AtomicI8::new(0),
                                               log_enabled: AtomicI8::new(1) };

impl EnableMap {
  pub fn error_enabled(&self) -> bool {
    self.error_enabled.load(Ordering::Relaxed) != 0
  }

  pub fn debug_enabled(&self) -> bool {
    self.debug_enabled.load(Ordering::Relaxed) != 0
  }

  pub fn info_enabled(&self) -> bool {
    self.info_enabled.load(Ordering::Relaxed) != 0
  }

  pub fn warning_enabled(&self) -> bool {
    self.warning_enabled.load(Ordering::Relaxed) != 0
  }

  pub fn log_enabled(&self) -> bool {
    self.log_enabled.load(Ordering::Relaxed) != 0
  }
}

// The log_impl macro calls println to output an actual
// log entry.  It is called by the macros intended for
// external use.
#[macro_export]
macro_rules! log_impl {
  ($level:ident, $module:ident, $($x:tt)+) => {
    println!("{}  {:10.10}  {:16.16}  {}",
      timestamp(), stringify!($level), stringify!($module), format!($($x)+));
  }
}

/// Write a log entry when enabled.
#[macro_export]
macro_rules! error {
    ($module:ident, $($x:tt)+) => {
      if $module.error_enabled() {
        log_impl!(error, $module, $($x)+);
      }
    }
}

/// Write a debug log entry when enabled.
#[macro_export]
macro_rules! debug {
    ($module:ident, $($x:tt)+) => {
      if $module.debug_enabled() {
        log_impl!(error, $module, $($x)+);
      }
    }
}

/// Write a debug log entry when enabled.
#[macro_export]
macro_rules! warning {
    ($module:ident, $($x:tt)+) => {
      if $module.warning_enabled() {
        log_impl!(error, $module, $($x)+);
      }
    }
}

/// Write a debug log entry when enabled.
#[macro_export]
macro_rules! info {
    ($module:ident, $($x:tt)+) => {
      if $module.info_enabled() {
        log_impl!(error, $module, $($x)+);
      }
    }
}

/// Write a log entry.
#[macro_export]
macro_rules! log {
    ($module:ident, $($x:tt)+) => {
      if $module.log_enabled() {
        log_impl!(error, $module, $($x)+);
      }
    }
}

/// Returns Some(Error::...).
#[macro_export]
macro_rules! se {
    ($($x:tt)+) => { Some(Error::new(ErrorKind::Other, format!($($x)+))) }
}

/// Returns Err(Error::new...).
#[macro_export]
macro_rules! er {
    ($($x:tt)+) => { Err(Error::new(ErrorKind::Other, format!($($x)+))) }
}

/// Returns a deserializer error:  Err(serde::de::Error::...)
#[macro_export]
macro_rules! sde  {
    ($($x:tt)+) => {
        Err(serde::de::Error::custom(format!($($x)+)))
    }
}

/// Produce a timestamp of UTC down to milliseconds, with rounding.
/// This routine ignores leap seconds.
pub fn timestamp() -> String {
  use chrono::DateTime;
  use chrono::Datelike;
  use chrono::Timelike;
  use chrono::Utc;

  let now: DateTime<Utc> = Utc::now();

  format!("{:04}/{:02}/{:02}  {:02}:{:02}:{:02}.{:03} UTC",
          now.year(),
          now.month(),
          now.day(),
          now.hour(),
          now.minute(),
          now.second(),
          (now.nanosecond() + 500 * 1000) / (1000 * 1000))
}

/// Convert a u64 into a string with commas.
fn commas_u64(input: u64) -> String {
  if input < 10000 {
    return format!("{}", input);
  }

  let mut value = input;
  let mut result = "".to_string();

  while value > 1000 {
    result = format!(",{:03.3}", value % 1000) + &result;
    value /= 1000;
  }

  if value == 1000 {
    result = "1,000".to_owned() + &result;
  } else {
    result = format!("{}", value) + &result;
  }

  result
}

/// Convert an i64 into a string with commas.
fn commas_i64(input: i64) -> String {
  if input == 0 {
    return "0".to_string();
  }

  let sign = input < 0;
  let mut result;

  if input == std::i64::MIN {
    result = commas_u64(1u64 << 63);
  } else if input < 0 {
    result = commas_u64(-input as u64);
  } else {
    result = commas_u64(input as u64);
  }

  if sign {
    result = "-".to_owned() + &result;
  }

  result
}

pub trait Commas {
  fn commas(self) -> String;
}

impl Commas for u64 {
  fn commas(self) -> String {
    crate::commas_u64(self)
  }
}

impl Commas for u32 {
  fn commas(self) -> String {
    crate::commas_u64(self as u64)
  }
}

impl Commas for u16 {
  fn commas(self) -> String {
    crate::commas_u64(self as u64)
  }
}

impl Commas for u8 {
  fn commas(self) -> String {
    crate::commas_u64(self as u64)
  }
}

impl Commas for usize {
  fn commas(self) -> String {
    crate::commas_u64(self as u64)
  }
}

impl Commas for i64 {
  fn commas(self) -> String {
    crate::commas_i64(self)
  }
}

impl Commas for i32 {
  fn commas(self) -> String {
    crate::commas_i64(self as i64)
  }
}

impl Commas for i16 {
  fn commas(self) -> String {
    crate::commas_i64(self as i64)
  }
}

impl Commas for i8 {
  fn commas(self) -> String {
    crate::commas_i64(self as i64)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_basic_logging() {
    let root = EnableMap { error_enabled: AtomicI8::new(1),
                           debug_enabled: AtomicI8::new(1),
                           warning_enabled: AtomicI8::new(1),
                           info_enabled: AtomicI8::new(1),
                           log_enabled: AtomicI8::new(1) };

    log!(root, "Here at {}", timestamp());
    info!(root, "Here at {}", timestamp());
    debug!(root, "Here at {}", timestamp());
    warning!(root, "Here at {}", timestamp());
    error!(root, "Here at {}", timestamp());
  }

  #[test]
  fn test_commas() {
    // Test u64
    assert_eq!("0", 0u64.commas());
    assert_eq!("100", 100u64.commas());
    assert_eq!("999", 999u64.commas());
    assert_eq!("1000", 1000_u64.commas());
    assert_eq!("9999", 9999u64.commas());
    assert_eq!("10,000", 10000_u64.commas());
    assert_eq!("1,000,000", (1000u64 * 1000u64).commas());
    assert_eq!("1,048,576", (1024 * 1024_u64).commas());
    assert_eq!("999,000", (999 * 1000_u64).commas());
    assert_eq!("2000", (2 * 1000_u64).commas());
    assert_eq!("1,000,000,000", (1000 * 1000 * 1000_u64).commas());
    assert_eq!("18,446,744,073,709,551,615", std::u64::MAX.commas());

    // Test u32
    assert_eq!("0", 0u32.commas());
    assert_eq!("100", 100u32.commas());
    assert_eq!("999", 999u32.commas());
    assert_eq!("1000", 1000_u32.commas());
    assert_eq!("9999", 9999u32.commas());
    assert_eq!("10,000", 10000_u32.commas());
    assert_eq!("1,000,000", (1000u32 * 1000u32).commas());
    assert_eq!("1,048,576", (1024 * 1024_u32).commas());
    assert_eq!("999,000", (999 * 1000_u32).commas());
    assert_eq!("2000", (2 * 1000_u32).commas());
    assert_eq!("1,000,000,000", (1000 * 1000 * 1000_u32).commas());
    assert_eq!("4,294,967,295", std::u32::MAX.commas());

    // Test u16
    assert_eq!("0", 0u16.commas());
    assert_eq!("100", 100u16.commas());
    assert_eq!("999", 999u16.commas());
    assert_eq!("1000", 1000_u16.commas());
    assert_eq!("9999", 9999u16.commas());
    assert_eq!("10,000", 10000_u16.commas());
    assert_eq!("2000", (2 * 1000_u16).commas());
    assert_eq!("65,535", std::u16::MAX.commas());

    // Test u8
    assert_eq!("0", 0u8.commas());
    assert_eq!("1", 1u8.commas());
    assert_eq!("100", 100u8.commas());
    assert_eq!("255", std::u8::MAX.commas());

    // Test i64
    assert_eq!("0", 0i64.commas());
    assert_eq!("100", 100i64.commas());
    assert_eq!("999", 999i64.commas());
    assert_eq!("1000", 1000.commas());
    assert_eq!("9999", 9999i64.commas());
    assert_eq!("10,000", 10000_i64.commas());
    assert_eq!("1,000,000", (1000i64 * 1000i64).commas());
    assert_eq!("999,000", (999i64 * 1000i64).commas());
    assert_eq!("2000", (2 * 1000_i64).commas());
    assert_eq!("1,000,000,000", (1000 * 1000 * 1000_i64).commas());
    assert_eq!("9,223,372,036,854,775,807", std::i64::MAX.commas());
    assert_eq!("-100", (-100_i64).commas());
    assert_eq!("-999", (-999_i64).commas());
    assert_eq!("-1000", (-1000_i64).commas());
    assert_eq!("-1,000,000", (-1000 * 1000_i64).commas());
    assert_eq!("-1,048,576", (-1024 * 1024_i64).commas());
    assert_eq!("-999,000", (-999 * 1000_i64).commas());
    assert_eq!("-2000", (-2 * 1000_i64).commas());
    assert_eq!("-1,000,000,000", (-1000 * 1000 * 1000_i64).commas());
    assert_eq!("-9,223,372,036,854,775,808", (std::i64::MIN).commas());

    // Test i32.
    assert_eq!("0", 0i32.commas());
    assert_eq!("100", 100i32.commas());
    assert_eq!("999", 999i32.commas());
    assert_eq!("1000", 1000.commas());
    assert_eq!("9999", 9999i32.commas());
    assert_eq!("10,000", 10000_i32.commas());
    assert_eq!("1,000,000", (1000i32 * 1000i32).commas());
    assert_eq!("999,000", (999i32 * 1000i32).commas());
    assert_eq!("2000", (2 * 1000_i32).commas());
    assert_eq!("1,000,000,000", (1000 * 1000 * 1000_i32).commas());
    assert_eq!("2,147,483,647", std::i32::MAX.commas());
    assert_eq!("-100", (-100_i32).commas());
    assert_eq!("-999", (-999_i32).commas());
    assert_eq!("-1000", (-1000_i32).commas());
    assert_eq!("-1,000,000", (-1000 * 1000_i32).commas());
    assert_eq!("-1,048,576", (-1024 * 1024_i32).commas());
    assert_eq!("-999,000", (-999 * 1000_i32).commas());
    assert_eq!("-2000", (-2 * 1000_i32).commas());
    assert_eq!("-1,000,000,000", (-1000 * 1000 * 1000_i32).commas());
    assert_eq!("-2,147,483,648", (std::i32::MIN).commas());

    // Test i16
    assert_eq!("0", 0i16.commas());
    assert_eq!("100", 100i16.commas());
    assert_eq!("999", 999i16.commas());
    assert_eq!("1000", 1000.commas());
    assert_eq!("9999", 9999i16.commas());
    assert_eq!("10,000", 10000_i16.commas());
    assert_eq!("2000", (2 * 1000_i16).commas());
    assert_eq!("32,767", std::i16::MAX.commas());
    assert_eq!("-100", (-100_i16).commas());
    assert_eq!("-999", (-999_i16).commas());
    assert_eq!("-1000", (-1000_i16).commas());
    assert_eq!("-2000", (-2 * 1000_i16).commas());
    assert_eq!("-32,768", (std::i16::MIN).commas());

    // Test i8
    assert_eq!("0", 0i8.commas());
    assert_eq!("-1", (-1i8).commas());
    assert_eq!("100", 100i8.commas());
    assert_eq!("127", std::i8::MAX.commas());
    assert_eq!("-100", (-100_i8).commas());
    assert_eq!("-128", (std::i8::MIN).commas());
  }
}