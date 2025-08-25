extern crate alloc;

use alloc::vec::Vec;

#[derive(Clone, Debug)]
pub enum HtmlTree<'a> {
    Tag(&'a str, Vec<(&'a str, &'a str)>),
    PTag(&'a str, Vec<(&'a str, &'a str)>, &'a HtmlTree<'a>),
}
