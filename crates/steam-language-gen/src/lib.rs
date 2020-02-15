#![allow(dead_code)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate num_derive;
#[macro_use]
extern crate steam_language_gen_derive;
#[macro_use]
extern crate arrayref;
#[macro_use]
extern crate derive_new;

pub mod generated;
pub mod generator;
pub mod parser;

#[derive(PartialEq, Eq)]
pub struct Token<'a> {
    value: String,
    default: Option<&'a str>,
}

impl<'a> Token<'a> {
    fn get_value(&self) -> &String {
        &self.value
    }

    fn get_default(&self) -> Option<&'a str> {
        self.default
    }
}

#[derive(PartialEq, Eq)]
pub enum Element {
    File,
    Head,
    Type,
    Member,
}
