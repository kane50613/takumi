//! Loads the CSS a document links: each sheet's relative `url()`s resolved against the sheet
//! itself, and its `@import` rules replaced by the sheets they import.

use std::fmt::Write;

use cssparser::{Delimiter, ParseError, Parser, ToCss, Token, serialize_string};
use url::Url;

use crate::sandbox::{ResourceError, Sandbox};

/// How deep `@import` rules may nest.
const MAX_IMPORT_DEPTH: usize = 16;

/// What loading a document's style sheets turned up besides the CSS.
#[derive(Debug, Default)]
pub(crate) struct LoadReport {
  /// Sheets that failed to load, which a browser skips as this does.
  pub(crate) skipped: Vec<ResourceError>,
}

/// Loads style sheets for one document through a sandbox.
pub(crate) struct StyleSheetLoader<'s> {
  sandbox: &'s Sandbox,
  report: LoadReport,
}

impl<'s> StyleSheetLoader<'s> {
  pub(crate) fn new(sandbox: &'s Sandbox) -> Self {
    Self {
      sandbox,
      report: LoadReport::default(),
    }
  }

  pub(crate) fn into_report(self) -> LoadReport {
    self.report
  }

  /// The CSS of the sheet at `url`, ready to parse with no base URL of its own. None when it
  /// fails to load, unless the failure [fails the render](ResourceError::fails_render).
  pub(crate) fn load(&mut self, url: &Url) -> Result<Option<String>, ResourceError> {
    self.load_nested(url, &mut Vec::new())
  }

  /// `css` written inside a document at `base`, with its `url()`s resolved and its imports loaded.
  pub(crate) fn rewrite(&mut self, css: &str, base: &Url) -> Result<String, ResourceError> {
    self.rewrite_nested(css, base, &mut Vec::new())
  }

  fn load_nested(
    &mut self,
    url: &Url,
    importing: &mut Vec<Url>,
  ) -> Result<Option<String>, ResourceError> {
    // A sheet that imports itself, directly or not, imports nothing the second time.
    if importing.contains(url) || importing.len() >= MAX_IMPORT_DEPTH {
      return Ok(Some(String::new()));
    }

    let bytes = match self.sandbox.read(url) {
      Ok(bytes) => bytes,
      Err(error) if error.fails_render() => return Err(error),
      Err(error) => {
        self.report.skipped.push(error);
        return Ok(None);
      }
    };
    let text = String::from_utf8_lossy(&bytes);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);

    importing.push(url.clone());
    let css = self.rewrite_nested(text, url, importing);
    importing.pop();

    css.map(Some)
  }

  fn rewrite_nested(
    &mut self,
    css: &str,
    base: &Url,
    importing: &mut Vec<Url>,
  ) -> Result<String, ResourceError> {
    let mut parser = Parser::new(css);
    let mut out = String::with_capacity(css.len());
    let mut imports_allowed = true;

    loop {
      let start = parser.state();
      let Ok(token) = parser.next_including_whitespace_and_comments() else {
        break;
      };
      let token = token.clone();

      match token {
        Token::WhiteSpace(_) | Token::Comment(_) | Token::CDO | Token::CDC => {
          out.push_str(parser.slice_from(start.position()));
        }
        Token::AtKeyword(name) if name.eq_ignore_ascii_case("import") => {
          let rule_start = start.position();
          let import = parser
            .parse_until_after(Delimiter::Semicolon, |input| {
              Ok::<_, ParseError<()>>(Import::parse(input, base))
            })
            .ok()
            .flatten();
          let rule = parser.slice_from(rule_start);

          match import {
            Some(import) if imports_allowed => match self.load_nested(&import.url, importing)? {
              Some(sheet) => import.wrap(sheet, &mut out),
              // A sheet that failed to load may be a Tailwind entry point, which takumi reads itself.
              None => out.push_str(rule),
            },
            Some(_) => {}
            None => out.push_str(rule),
          }
        }
        Token::AtKeyword(name)
          if name.eq_ignore_ascii_case("charset") || name.eq_ignore_ascii_case("layer") =>
        {
          let is_layer = name.eq_ignore_ascii_case("layer");

          out.push_str(parser.slice_from(start.position()));
          let ends_in_block = rewrite_rest(&mut parser, base, &mut out);

          // `@layer a, b;` may precede imports, but a layer block is a rule that ends them.
          imports_allowed &= !(is_layer && ends_in_block);
        }
        _ => {
          parser.reset(&start);
          imports_allowed = false;
          rewrite_rest(&mut parser, base, &mut out);
        }
      }
    }

    Ok(out)
  }
}

/// Copies the rest of one top-level statement or rule to `out`, through its `;` or block, with
/// its `url()`s resolved against `base`. Whether it ended in a block.
fn rewrite_rest(parser: &mut Parser<'_>, base: &Url, out: &mut String) -> bool {
  let mut ends_in_block = false;

  while let Ok(token) = parser.next_including_whitespace_and_comments() {
    let token = token.clone();
    let is_end = matches!(token, Token::Semicolon | Token::CurlyBracketBlock);

    write_token(parser, &token, base, false, out);

    if is_end {
      ends_in_block = matches!(token, Token::CurlyBracketBlock);
      break;
    }
  }

  ends_in_block
}

/// Writes `token` to `out`, resolving it as a URL if it is one, and the block it opens.
fn write_token(
  parser: &mut Parser<'_>,
  token: &Token<'_>,
  base: &Url,
  in_image_set: bool,
  out: &mut String,
) {
  match token {
    Token::UnquotedUrl(url) => write_url(url, base, out),
    Token::QuotedString(url) if in_image_set => {
      let _ = serialize_string(&resolve(url, base), out);
    }
    Token::Function(name)
      if name.eq_ignore_ascii_case("url") || name.eq_ignore_ascii_case("src") =>
    {
      let argument = parser.parse_nested_block(|input| {
        let start = input.state();

        if let Ok(url) = input.expect_string_cloned() {
          return Ok::<_, ParseError<()>>(Ok(url.to_string()));
        }

        input.reset(&start);
        while input.next_including_whitespace_and_comments().is_ok() {}

        Ok(Err(input.slice_from(start.position()).to_owned()))
      });

      match argument {
        Ok(Ok(url)) => write_url(&url, base, out),
        Ok(Err(raw)) => {
          let _ = token.to_css(out);
          let _ = write!(out, "{raw})");
        }
        Err(_) => {
          let _ = token.to_css(out);
          out.push(')');
        }
      }
    }
    Token::Function(_)
    | Token::ParenthesisBlock
    | Token::SquareBracketBlock
    | Token::CurlyBracketBlock => {
      let image_set = matches!(token, Token::Function(name)
        if name.eq_ignore_ascii_case("image-set") || name.eq_ignore_ascii_case("-webkit-image-set"));

      let _ = token.to_css(out);
      let _ = parser.parse_nested_block(|input| {
        while let Ok(inner) = input.next_including_whitespace_and_comments() {
          let inner = inner.clone();

          write_token(input, &inner, base, image_set, out);
        }

        Ok::<_, ParseError<()>>(())
      });
      out.push(match token {
        Token::SquareBracketBlock => ']',
        Token::CurlyBracketBlock => '}',
        _ => ')',
      });
    }
    _ => {
      let _ = token.to_css(out);
    }
  }
}

fn write_url(url: &str, base: &Url, out: &mut String) {
  out.push_str("url(");
  let _ = serialize_string(&resolve(url, base), out);
  out.push(')');
}

/// `url` resolved against `base`, or as written when it does not resolve, as a fragment-only
/// reference into an SVG document does not.
fn resolve(url: &str, base: &Url) -> String {
  if url.starts_with('#') {
    return url.to_owned();
  }

  base.join(url).map_or_else(|_| url.to_owned(), String::from)
}

/// An `@import` rule: the sheet it loads and the conditions it applies the sheet under.
struct Import {
  url: Url,
  layer: Option<String>,
  supports: Option<String>,
  media: String,
}

impl Import {
  /// The prelude after `@import`, none when it names no URL.
  fn parse(input: &mut Parser<'_>, base: &Url) -> Option<Self> {
    let target = match input.next().ok()?.clone() {
      Token::QuotedString(url) | Token::UnquotedUrl(url) => url.to_string(),
      Token::Function(name) if name.eq_ignore_ascii_case("url") => input
        .parse_nested_block(|input| input.expect_string_cloned().map_err(ParseError::<()>::from))
        .ok()?
        .to_string(),
      _ => return None,
    };
    let url = base.join(&target).ok()?;
    let mut layer = None;
    let mut supports = None;

    loop {
      let state = input.state();

      match input.next() {
        Ok(Token::Ident(name)) if name.eq_ignore_ascii_case("layer") && layer.is_none() => {
          layer = Some(String::new());
        }
        Ok(Token::Function(name)) if name.eq_ignore_ascii_case("layer") && layer.is_none() => {
          layer = Some(nested_text(input));
        }
        Ok(Token::Function(name))
          if name.eq_ignore_ascii_case("supports") && supports.is_none() =>
        {
          supports = Some(nested_text(input));
        }
        _ => {
          input.reset(&state);
          break;
        }
      }
    }

    let media_start = input.position();

    while input.next().is_ok() {}

    let media = input
      .slice_from(media_start)
      .trim()
      .trim_end_matches(';')
      .trim()
      .to_owned();

    Some(Self {
      url,
      layer,
      supports,
      media,
    })
  }

  /// Writes `sheet` to `out` inside the rules that apply this import's conditions.
  fn wrap(&self, sheet: String, out: &mut String) {
    let mut closing = 0;

    if !self.media.is_empty() {
      let _ = write!(out, "@media {} {{", self.media);
      closing += 1;
    }
    if let Some(supports) = &self.supports {
      let condition = if supports.trim_start().starts_with('(') {
        supports.clone()
      } else {
        format!("({supports})")
      };
      let _ = write!(out, "@supports {condition} {{");
      closing += 1;
    }
    if let Some(layer) = &self.layer {
      let _ = write!(out, "@layer {layer} {{");
      closing += 1;
    }

    out.push_str(&sheet);
    out.push_str(&"}".repeat(closing));
  }
}

/// The source text inside the block the parser just opened.
fn nested_text(input: &mut Parser<'_>) -> String {
  input
    .parse_nested_block(|input| {
      let start = input.position();

      while input.next_including_whitespace_and_comments().is_ok() {}

      Ok::<_, ParseError<()>>(input.slice_from(start).trim().to_owned())
    })
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
  use std::{fs, path::PathBuf};

  use super::*;
  use crate::sandbox::directory_url;

  fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("takumi-cli-css-{name}-{}", std::process::id()));

    fs::create_dir_all(dir.join("css")).unwrap();

    dir
  }

  #[test]
  fn resolves_urls_against_the_sheet() {
    let root = temp_dir("urls");

    fs::write(
      root.join("css/a.css"),
      r#"a { background: url(img/a.png), url("/b.png") } @font-face { src: url('../f.woff2') } .m { mask: url(#m) } b { background: image-set("c.png" 1x) }"#,
    )
    .unwrap();

    let sandbox = Sandbox::new([root.clone()]).unwrap();
    let base = directory_url(&root).unwrap();
    let css = StyleSheetLoader::new(&sandbox)
      .load(&base.join("css/a.css").unwrap())
      .unwrap()
      .unwrap();

    assert!(
      css.contains(&format!(r#"url("{base}css/img/a.png")"#)),
      "{css}"
    );
    assert!(css.contains(r#"url("file:///b.png")"#), "{css}");
    assert!(css.contains(&format!(r#"url("{base}f.woff2")"#)), "{css}");
    assert!(css.contains(r##"url("#m")"##), "{css}");
    assert!(
      css.contains(&format!(r#"image-set("{base}css/c.png" 1x)"#)),
      "{css}"
    );
  }

  #[test]
  fn inlines_imports_under_their_conditions() {
    let root = temp_dir("imports");

    fs::write(
      root.join("css/main.css"),
      r#"@charset "utf-8"; @layer base; @import "b.css" layer(base) supports(display: grid) print; @import url(c.css); a{} @import "late.css";"#,
    )
    .unwrap();
    fs::write(root.join("css/b.css"), "b { background: url(b.png) }").unwrap();
    fs::write(root.join("css/c.css"), "@import 'main.css'; c{}").unwrap();
    fs::write(root.join("css/late.css"), "late{}").unwrap();

    let sandbox = Sandbox::new([root.clone()]).unwrap();
    let base = directory_url(&root).unwrap();
    let css = StyleSheetLoader::new(&sandbox)
      .load(&base.join("css/main.css").unwrap())
      .unwrap()
      .unwrap();

    assert!(
      css.contains(&format!(
        r#"@media print {{@supports (display: grid) {{@layer base {{b {{ background: url("{base}css/b.png") }}}}}}}}"#
      )),
      "{css}"
    );
    assert!(css.contains(" c{}"), "{css}");
    assert!(!css.contains("late{}"), "{css}");
  }

  #[test]
  fn a_missing_import_stays_for_takumi_to_read() {
    let root = temp_dir("tailwind");
    let sandbox = Sandbox::new([root.clone()]).unwrap();
    let base = directory_url(&root).unwrap();
    let mut loader = StyleSheetLoader::new(&sandbox);
    let css = loader
      .rewrite(r#"@import "tailwindcss"; a{}"#, &base)
      .unwrap();

    assert!(css.starts_with(r#"@import "tailwindcss";"#), "{css}");
    assert_eq!(loader.into_report().skipped.len(), 1);
  }

  #[test]
  fn an_import_outside_the_roots_fails_the_load() {
    let root = temp_dir("denied");
    let sandbox = Sandbox::new([root.join("css")]).unwrap();
    let base = directory_url(&root.join("css")).unwrap();

    fs::write(root.join("secret.css"), "s{}").unwrap();

    assert!(matches!(
      StyleSheetLoader::new(&sandbox).rewrite(r#"@import "../secret.css";"#, &base),
      Err(ResourceError::Denied { .. })
    ));
  }
}
