pub mod ast;
lalrpop_mod!(html, "/html/html.rs");

extern crate alloc;

use core::error::Error;

use alloc::{boxed::Box, string::ToString};
use ast::HtmlTree;
use html::TAGParser;
use lalrpop_util::lalrpop_mod;
use typed_arena::Arena;

pub(crate) fn parse<'a>(
    arena: &'a Arena<HtmlTree<'a>>,
    htmlfile: &'a str,
) -> Result<&'a HtmlTree<'a>, Box<dyn Error + 'a>> {
    let htmlparse = TAGParser::new();

    match htmlparse.parse(arena, htmlfile) {
        Ok(v) => Ok(v),
        Err(e) => return Err(Box::<dyn Error>::from(e.to_string())),
    }
}
