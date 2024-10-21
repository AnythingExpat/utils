use core::fmt;
use std::{ffi::OsString, iter::once};

extern crate self as utils;

#[cfg(feature = "derive")]
pub use utils_derive::*;

#[derive(fmt::Debug)]
pub enum EnvErrorType {
    NotPresent,
    NotUnicode(OsString),
    InvalidFormat,
    Other(String)
}

#[derive(fmt::Debug)]
pub struct EnvError {
    pub var: String,
    pub ty: EnvErrorType
}

impl From<std::env::VarError> for EnvErrorType {
    fn from(value: std::env::VarError) -> Self {
        match value {
            std::env::VarError::NotPresent => EnvErrorType::NotPresent,
            std::env::VarError::NotUnicode(str) => EnvErrorType::NotUnicode(str)
        }
    }
}

impl fmt::Display for EnvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error parsing environment variable '{}': ", self.var)?;
        match &self.ty {
            EnvErrorType::NotPresent => write!(f, "Not present"),
            EnvErrorType::NotUnicode(_) => write!(f, "Not valid unicode"),
            EnvErrorType::InvalidFormat => write!(f, "Unable to parse"),
            EnvErrorType::Other(err) => write!(f, "{}", err)
        }
    }
}

impl EnvError {
    fn convert<T, Err: Into<EnvErrorType>>(res: Result<T, Err>, ident: &str) -> Result<T, EnvError> {
        res.map_err(|err| EnvError { var: String::from(ident), ty: err.into() })
    }
}

pub trait FromEnv where Self: Sized {
    fn from_env(value: &str) -> Result<Self, EnvErrorType>;
    fn load(ident: &str) -> Result<Self, EnvError> {
        EnvError::convert(Self::from_env(&EnvError::convert(std::env::var(ident), ident)?), ident)
    }

    fn load_or_file(ident: &str) -> Result<Self, EnvError> {
        let str = match std::env::var(ident) {
            Ok(value) => value,
            Err(std::env::VarError::NotPresent) => {
                let name = format!("{}_FILE", ident);
                std::fs::read_to_string(EnvError::convert(std::env::var(&name), &name)?).map_err(|err| EnvError { var: name, ty: EnvErrorType::Other(err.to_string()) })?
            },
            Err(err) => { return Err(EnvError { var: String::from(ident), ty: err.into() }); }
        };

        EnvError::convert(Self::from_env(&str), ident)
    }
}

macro_rules! impl_from_env {
    ($($t:ty),*) => {
        $(impl FromEnv for $t {
            fn from_env(value: &str) -> Result<Self, EnvErrorType> {
                value.parse().map_err(|_| EnvErrorType::InvalidFormat)
            }
        })*
    };
}

impl_from_env!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);
impl_from_env!(f32, f64, bool, String);

impl<T> FromEnv for Option<T> where T: FromEnv {
    fn from_env(value: &str) -> Result<Self, EnvErrorType> {
        Ok(Some(T::from_env(value)?))
    }

    fn load(ident: &str) -> Result<Self, EnvError> {
        match std::env::var(ident) {
            Ok(value) => EnvError::convert(FromEnv::from_env(&value), ident),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(err) => Err(EnvError { var: String::from(ident), ty: err.into() })
        }
    }

    fn load_or_file(ident: &str) -> Result<Self, EnvError> {
        let str = match std::env::var(ident) {
            Ok(value) => value,
            Err(std::env::VarError::NotPresent) => match std::env::var(format!("{}_FILE", ident)) {
                Ok(path) => std::fs::read_to_string(path).map_err(|err| EnvError { var: String::from(ident), ty: EnvErrorType::Other(err.to_string()) })?,
                Err(std::env::VarError::NotPresent) => { return Ok(None); },
                Err(err) => { return Err(EnvError { var: String::from(ident), ty: err.into() }); }
            },
            Err(err) => { return Err(EnvError { var: String::from(ident), ty: err.into() }); }
        };

        EnvError::convert(FromEnv::from_env(&str), ident)
    }
}

pub struct Masked<T>(pub T);

impl<T> FromEnv for Masked<T> where T: FromEnv {
    fn from_env(value: &str) -> Result<Self, EnvErrorType> {
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