#![feature(box_syntax)]
#![feature(box_patterns)]
#![allow(dead_code)]

#[macro_use]
extern crate num_derive;
#[macro_use]
extern crate steam_language_gen_derive;
#[macro_use]
extern crate serde;

#[macro_use]
extern crate bitflags;

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
