use proc_macro::TokenStream;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use quote::quote;

use super::GLOBAL_STYLE;

//  styling idea:
//  caller passes in mapping of tag to yew component name
//      caller can implement component however they want, it can take children
//  instead of rendering <p> we render <SpecialP>
//  problem: has to be specified at every call-site, and it's verbose because it has to be the input
// to the proc macro
//
//  take 2: use yew dynamic tags to lookup in a named or global style array,
//  including fallbacks to defaults. how to control whether style is applied at
//  all? could have 2 different mdx macros, mdx/mdxs (global style) / mdxss
//  (user-specified name of style map)
//  Disadvantage: yew component name validation done at runtime
//  instead of mapping to component name, could just map to Fn(children)-> Html
//  dealbreaker?: do dynamic tags work with components anyways??
//
//  take2.5: just dont use dynamic tag?? -- but then we need the map specified at compile-time
//
//  take 3: separate mdx_style! macro to define styles

//  map static tag to dynamic tag, falling back to the given tag
#[derive(PartialEq)]
enum Side {
    Start,
    End,
}

fn dyn_tag_name_opt(tag: &str) -> Option<String> {
    GLOBAL_STYLE.lock().unwrap().get(tag).map(Into::into)
}
fn dyn_tag_name(tag: &str) -> String {
    dyn_tag_name_opt(tag).unwrap_or(tag.into())
}

fn dyn_tag(tag: &str, side: Side) -> TokenStream {
    let tag = dyn_tag_name_opt(tag).unwrap_or(tag.into());
    (match side {
        Side::Start => "<",
        Side::End => "</",
    }
    .to_string()
        + &tag
        + ">")
        .parse()
        .unwrap()
}

fn dyn_tag_opt(tag: &str, side: Side) -> Option<TokenStream> {
    GLOBAL_STYLE.lock().unwrap().get(tag.into()).map(|tag| {
        (match side {
            Side::Start => "<",
            Side::End => "</",
        }
        .to_string()
            + &tag
            + ">")
            .parse()
            .unwrap()
    })
}

pub fn parse_commonmark(input: &str) -> TokenStream {
    let parser = Parser::new_ext(input, Options::all());

    let mut toks = TokenStream::new();
    toks.extend::<TokenStream>("<>".parse().unwrap());

    parser.for_each(|evt| {
        // dbg!(&evt);
        let new_toks: TokenStream = match evt {
            Event::Start(tag) => match tag {
                Tag::Paragraph => dyn_tag("p", Side::Start),
                Tag::Heading { level, .. } => dyn_tag(&level.to_string(), Side::Start),
                Tag::BlockQuote(_) => dyn_tag("blockquote", Side::Start),
                Tag::CodeBlock(kind) => match kind {
                    pulldown_cmark::CodeBlockKind::Indented => FromIterator::from_iter(
                        [
                            dyn_tag("pre", Side::Start),
                            "<code>".parse::<TokenStream>().unwrap(),
                        ]
                        .into_iter(),
                    ),
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => {
                        let tags = FromIterator::from_iter(
                            [
                                dyn_tag("pre", Side::Start),
                                format!("<code class=\"language-{}\">", lang)
                                    .parse::<TokenStream>()
                                    .unwrap(),
                            ]
                            .into_iter(),
                        );

                        tags
                    }
                },
                Tag::List(None) => dyn_tag("ul", Side::Start),
                Tag::List(Some(0)) => dyn_tag("ol", Side::Start),
                Tag::List(Some(0..=u64::MAX)) => dyn_tag("ol", Side::Start),
                Tag::Item => dyn_tag("li", Side::Start),
                Tag::FootnoteDefinition(_) => todo!(),
                Tag::Table(_) => dyn_tag("table", Side::Start),
                Tag::TableHead => dyn_tag("thead", Side::Start),
                Tag::TableRow => dyn_tag("tr", Side::Start),
                Tag::TableCell => dyn_tag("td", Side::Start),
                Tag::Emphasis => dyn_tag("em", Side::Start),
                Tag::Strong => dyn_tag("strong", Side::Start),
                Tag::Strikethrough => dyn_tag("s", Side::Start),
                Tag::Link { dest_url, title, .. } => {
                    format!("<{} href=\"{}\">{}", dyn_tag_name("a").to_string(), dest_url, title)
                        .parse()
                        .unwrap()
                }
                Tag::Image { dest_url, title, .. } => {
                    let tag = dyn_tag_name("img");
                    format!(r#"<{tag} src="{dest_url}" title="{title}"/>"#)
                        .parse()
                        .unwrap()
                }
                _ => quote! {}.into(),
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph => dyn_tag("p", Side::End),
                TagEnd::Heading(level) => dyn_tag(&level.to_string(), Side::End),
                TagEnd::BlockQuote(_) => dyn_tag("blockquote", Side::End),
                TagEnd::CodeBlock => {
                    FromIterator::from_iter(["</code>".parse().unwrap(), dyn_tag("pre", Side::End)])
                }
                TagEnd::List(false) => dyn_tag("ul", Side::End),
                TagEnd::List(true) => dyn_tag("ol", Side::End),
                TagEnd::Item => dyn_tag("li", Side::End),
                TagEnd::FootnoteDefinition => todo!(),
                TagEnd::Table => dyn_tag("table", Side::End),
                TagEnd::TableHead => dyn_tag("thead", Side::End),
                TagEnd::TableRow => dyn_tag("tr", Side::End),
                TagEnd::TableCell => dyn_tag("td", Side::End),
                TagEnd::Emphasis => dyn_tag("em", Side::End),
                TagEnd::Strong => dyn_tag("strong", Side::End),
                TagEnd::Strikethrough => dyn_tag("s", Side::End),
                TagEnd::Link => dyn_tag("a", Side::End),
                TagEnd::Image => "".parse().unwrap(),
                _ => quote! {}.into(),
            },
            Event::Text(txt) => format!("{{r###\"{}\"###}}", txt).parse().unwrap(),
            Event::Code(code) => {
                let tag = dyn_tag_name("code");
                format!("<{tag}>{{r###\"{}\"###}}</{tag}>", code)
                    .parse()
                    .unwrap()
            }
            Event::Rule => {
                let tag = dyn_tag_name("hr");
                format!("<{tag} />").parse().unwrap()
            }
            Event::SoftBreak => "{{\" \"}}".parse().unwrap(),
            Event::HardBreak => {
                let tag = dyn_tag_name("br");
                format!("<{tag} />").parse().unwrap()
            },
            Event::Html(html) => html.parse().unwrap(),
            _ => quote! {}.into(),
        };
        toks.extend(new_toks);
    });

    toks.extend::<TokenStream>("</>".parse().unwrap());

    toks
}
