use core::fmt;
use std::{ffi::OsString, iter::once};

extern crate self as utils;

#[cfg(feature = "derive")]
pub use utils_derive::*;

#[derive(fmt::Debug)]
pub enum EnvError {
    NotPresent,
    NotUnicode(OsString),
    InvalidFormat,
    Other(String)
}

impl From<std::env::VarError> for EnvError {
    fn from(value: std::env::VarError) -> Self {
        match value {
            std::env::VarError::NotPresent => EnvError::NotPresent,
            std::env::VarError::NotUnicode(str) => EnvError::NotUnicode(str)
        }
    }
}

pub trait FromEnv where Self: Sized {
    fn from_env(value: &str) -> Result<Self, EnvError>;
    fn load(ident: &str) -> Result<Self, EnvError> {
        Self::from_env(&std::env::var(ident)?)
    }

    fn load_or_file(ident: &str) -> Result<Self, EnvError> {
        let str = match std::env::var(ident) {
            Ok(value) => value,
            Err(std::env::VarError::NotPresent) => std::fs::read_to_string(std::env::var(format!("{}_FILE", ident))?).map_err(|err| EnvError::Other(err.to_string()))?,
            Err(err) => { return Err(err.into()); }
        };

        Self::from_env(&str)
    }
}

macro_rules! impl_from_env {
    ($($t:ty),*) => {
        $(impl FromEnv for $t {
            fn from_env(value: &str) -> Result<Self, EnvError> {
                value.parse().map_err(|_| EnvError::InvalidFormat)
            }
        })*
    };
}

impl_from_env!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);
impl_from_env!(f32, f64, bool, String);

impl<T> FromEnv for Option<T> where T: FromEnv {
    fn from_env(value: &str) -> Result<Self, EnvError> {
        Ok(Some(T::from_env(value)?))
    }

    fn load(ident: &str) -> Result<Self, EnvError> {
        match std::env::var(ident) {
            Ok(value) => Ok(FromEnv::from_env(&value)?),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(err) => Err(err.into())
        }
    }

    fn load_or_file(ident: &str) -> Result<Self, EnvError> {
        let str = match std::env::var(ident) {
            Ok(value) => value,
            Err(std::env::VarError::NotPresent) => match std::env::var(format!("{}_FILE", ident)) {
                Ok(path) => std::fs::read_to_string(path).map_err(|err| EnvError::Other(err.to_string()))?,
                Err(std::env::VarError::NotPresent) => { return Ok(None); },
                Err(err) => { return Err(err.into()); }
            },
            Err(err) => { return Err(err.into()); }
        };

        FromEnv::from_env(&str)
    }
}

pub struct Masked<T>(pub T);

impl<T> FromEnv for Masked<T> where T: FromEnv {
    fn from_env(value: &str) -> Result<Self, EnvError> {
        T::from_env(value).map(|val| Masked(val))
    }
}

impl<T> From<T> for Masked<T> {
    fn from(value: T) -> Self {
        Masked(value)
    }
}

impl<T> fmt::Debug for Masked<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "***")
    }
}

impl<T> fmt::Display for Masked<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "***")
    }
}

fn __convert_ident(ident: impl Iterator<Item = char>) -> impl Iterator<Item = char> {
    ident.flat_map(|ch| ch.to_uppercase())
}

pub fn __join_idents(ident: &str, postfix: &str) -> String {
    if ident.is_empty() {
        __convert_ident(postfix.chars())
            .collect()
    } else {
        __convert_ident(ident.chars())
            .chain(once('_'))
            .chain(__convert_ident(postfix.chars()))
            .collect()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[cfg(feature = "derive")]
    #[derive(FromEnv, Debug)]
    struct NestedConfig {
        host: String,
        address: Option<Masked<String>>
    }
    
    #[cfg(feature = "derive")]
    #[derive(FromEnv, Debug)]
    struct TestConfig {
        id: i32,
        #[utils(var_or_file, name = "TEST_NAME")]
        name: String,
        guest_id: Option<u64>,
        nested: NestedConfig
    }

    #[cfg(feature = "derive")]
    #[test]
    fn test_regular() {
        std::env::set_var("ID", "5");
        std::env::remove_var("TEST_NAME_FILE");
        std::env::set_var("TEST_NAME", "john doe");
        std::env::set_var("GUEST_ID", "4");
        std::env::set_var("NESTED_HOST", "yoyo");

        let config = TestConfig::load("").expect("Config should parse correctly");
        assert_eq!(config.id, 5);
        assert_eq!(config.name, "john doe");
        assert_eq!(config.guest_id, Some(4));
        assert_eq!(config.nested.host, "yoyo");
        assert!(config.nested.address.is_none());
    }

    #[cfg(feature = "derive")]
    #[test]
    fn test_file() {
        std::env::set_var("ID", "5");
        std::env::set_var("GUEST_ID", "4");
        std::env::set_var("NESTED_HOST", "yoyo");
        std::env::remove_var("TEST_NAME");
        std::env::set_var("TEST_NAME_FILE", "test_name.txt");

        let config2 = TestConfig::load("").expect("Config should parse correctly");
        assert_eq!(config2.name, "test");
    }
}