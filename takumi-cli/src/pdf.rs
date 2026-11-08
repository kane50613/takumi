//! `takumi pdf`: the document paged into a PDF.

use std::{
  error::Error,
  path::{Path, PathBuf},
  sync::Arc,
};

use takumi::prelude::{
  Color, ColorInput, Fonts, FromCssStr, PageName, RenderNeeds, StyleSheet, Viewport,
  page_rule::{PageContext, PageKind},
};
use takumi_pdf::{MeasureOptions, PageOptions, PageRange, PdfMetadata, PdfOptions};

use crate::{
  document::Document,
  input::{InputArgs, OutputArgs, UsageError},
  product::{Images, Product},
};

/// Renders an HTML file to a paged PDF.
#[derive(clap::Args)]
#[command(after_help = "Examples:
  takumi pdf invoice.html -o invoice.pdf
  takumi pdf report.html -o report.pdf --page \"letter landscape\" --margin \"0.5in\"
  takumi pdf book.html -o excerpt.pdf --pages 1-3,8 --outline
  takumi pdf report.html -o report.pdf --margin \"2cm 1cm\" --footer footer.html

A header or footer fills elements with the class `pageNumber` or `totalPages`
with the page number or the page count, as in
  <div style=\"text-align: center\"><span class=\"pageNumber\"></span> / <span class=\"totalPages\"></span></div>")]
pub(crate) struct PdfArgs {
  #[command(flatten)]
  pub(crate) input: InputArgs,

  #[command(flatten)]
  pub(crate) output: OutputArgs,

  /// The page size as the CSS `size` property takes it, such as `A4`, `letter landscape`, or
  /// `210mm 297mm`. Defaults to A4. The document's `@page` rule wins.
  #[arg(long, value_name = "SIZE")]
  page: Option<String>,

  /// The page margins as the CSS `margin` property takes them, such as `20mm` or `1in 0.75in`.
  /// Defaults to 1cm. The document's `@page` rule wins.
  #[arg(long, value_name = "MARGIN")]
  margin: Option<String>,

  /// The pages to keep, such as `1-3,8` or `5-`. Page numbers still count every page.
  #[arg(long, value_name = "RANGES", value_parser = parse_page_ranges)]
  pages: Option<PageRanges>,

  /// A CSS color to fill every page with, margins included.
  #[arg(long, value_name = "COLOR", value_parser = parse_color)]
  background: Option<Color>,

  /// The document title the PDF viewer shows.
  #[arg(long)]
  title: Option<String>,

  /// An author of the document. Repeat it for more.
  #[arg(long = "author", value_name = "NAME")]
  authors: Vec<String>,

  /// Adds bookmarks for the document's `h1` to `h6` headings.
  #[arg(long)]
  outline: bool,

  /// An HTML file drawn in the top margin of every page. Its references resolve against the
  /// file and read through the same directories as the input's.
  #[arg(long, value_name = "FILE")]
  header: Option<PathBuf>,

  /// An HTML file drawn in the bottom margin of every page, as `--header` is in the top one.
  #[arg(long, value_name = "FILE")]
  footer: Option<PathBuf>,
}

/// The `--pages` spans, in a type clap can hold.
#[derive(Clone)]
struct PageRanges(Vec<PageRange>);

/// A PDF product with its settings checked.
pub(crate) struct Pdf {
  page_css: Option<String>,
  header: Option<PathBuf>,
  footer: Option<PathBuf>,
  page_ranges: Option<Vec<PageRange>>,
  background: Option<Color>,
  metadata: Option<PdfMetadata>,
  outline: bool,
}

impl PdfArgs {
  pub(crate) fn product(&self) -> Result<Pdf, Box<dyn Error>> {
    if let Some(extension) = self.output.extension()
      && extension != "pdf"
    {
      return Err(Box::new(UsageError(format!(
        "`takumi pdf` writes PDF, but the output ends in `.{extension}`. Name it `.pdf` or use `takumi image`."
      ))));
    }

    let declarations: Vec<String> = [("size", &self.page), ("margin", &self.margin)]
      .into_iter()
      .filter_map(|(property, value)| {
        value
          .as_ref()
          .map(|value| check_page_declaration(property, value))
      })
      .collect::<Result<_, _>>()
      .map_err(UsageError)?;
    let metadata = (self.title.is_some() || !self.authors.is_empty()).then(|| PdfMetadata {
      title: self.title.clone(),
      authors: self.authors.clone(),
      ..PdfMetadata::default()
    });

    Ok(Pdf {
      header: self.header.clone(),
      footer: self.footer.clone(),
      page_css: (!declarations.is_empty())
        .then(|| format!("@page {{ {} }}", declarations.join("; "))),
      page_ranges: self.pages.clone().map(|ranges| ranges.0),
      background: self.background,
      metadata,
      outline: self.outline,
    })
  }
}

/// `property: value` for an `@page` rule, if takumi reads the value. A value holding anything
/// that could end the declaration or the rule, or raise it to `!important`, is refused, so a flag
/// stays one default the document's own `@page` rule overrides.
fn check_page_declaration(property: &str, value: &str) -> Result<String, String> {
  if value.contains(['{', '}', ';', '!', '@', '\\']) || value.contains("/*") {
    return Err(format!(
      "`{value}` is not a CSS `{property}` value takumi reads."
    ));
  }

  let declaration = format!("{property}: {value}");
  let descriptors = StyleSheet::parse_loosy(&format!("@page {{ {declaration} }}"))
    .page_descriptors(
      Viewport::new((800, 600)),
      PageKind::First,
      &PageName::default(),
      &PageContext::default(),
    );
  let is_read = match property {
    "size" => descriptors.size.is_some(),
    _ => descriptors.margin.top.is_some(),
  };

  if is_read {
    Ok(declaration)
  } else {
    Err(format!(
      "`{value}` is not a CSS `{property}` value takumi reads."
    ))
  }
}

/// `--pages`: comma-separated pages and spans, such as `1-3,8` or `5-`.
fn parse_page_ranges(value: &str) -> Result<PageRanges, String> {
  let page = |page: &str| -> Result<Option<usize>, String> {
    let page = page.trim();

    if page.is_empty() {
      return Ok(None);
    }

    match page.parse::<usize>() {
      Ok(page) if page > 0 => Ok(Some(page)),
      _ => Err(format!(
        "`{page}` is not a page number. Pages count from 1."
      )),
    }
  };

  value
    .split(',')
    .map(|span| {
      let range = match span.split_once('-') {
        Some((from, to)) => PageRange {
          from: page(from)?,
          to: page(to)?,
        },
        None => PageRange::single(page(span)?.ok_or("a page range is empty.")?),
      };

      match (range.from, range.to) {
        (Some(from), Some(to)) if from > to => Err(format!("`{span}` runs backwards.")),
        _ => Ok(range),
      }
    })
    .collect::<Result<_, _>>()
    .map(PageRanges)
}

fn parse_color(value: &str) -> Result<Color, String> {
  ColorInput::from_css_str(value)
    .map(|color| color.resolve(Color::black()))
    .map_err(|_| format!("`{value}` is not a CSS color."))
}

impl Product for Pdf {
  fn default_css(&self) -> Option<String> {
    self.page_css.clone()
  }

  fn band_files(&self) -> Vec<&Path> {
    self
      .header
      .iter()
      .chain(&self.footer)
      .map(PathBuf::as_path)
      .collect()
  }

  fn needs(
    &self,
    document: &Document,
    fonts: &Fonts,
    images: &Images,
  ) -> Result<RenderNeeds, Box<dyn Error>> {
    Ok(takumi_pdf::needs(
      MeasureOptions::builder()
        .fonts(fonts)
        .node(document.node.clone())
        .stylesheet(Arc::clone(&document.stylesheet))
        .images(images.clone())
        .page(PageOptions::A4)
        .build(),
    )?)
  }

  fn render(
    &self,
    document: Document,
    bands: Vec<Document>,
    fonts: &Fonts,
    images: Images,
  ) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bands = bands.into_iter().map(|band| band.node);
    let header = self.header.as_ref().and_then(|_| bands.next());
    let footer = self.footer.as_ref().and_then(|_| bands.next());
    let mut options = PdfOptions::builder()
      .fonts(fonts)
      .node(document.node)
      .stylesheet(document.stylesheet)
      .images(images)
      .page(PageOptions::A4)
      .build();

    options.page_ranges = self.page_ranges.clone();
    options.background_color = self.background;
    options.metadata = self.metadata.clone();
    options.outline = self.outline;
    options.header = header;
    options.footer = footer;

    Ok(takumi_pdf::render(options)?)
  }

  fn is_text(&self) -> bool {
    false
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn reads_pages_and_spans() {
    let ranges = parse_page_ranges("1-3, 8,5-,-2").unwrap().0;

    assert_eq!(
      ranges,
      [
        PageRange {
          from: Some(1),
          to: Some(3)
        },
        PageRange::single(8),
        PageRange {
          from: Some(5),
          to: None
        },
        PageRange {
          from: None,
          to: Some(2)
        },
      ]
    );
    assert!(parse_page_ranges("0").is_err());
    assert!(parse_page_ranges("3-1").is_err());
    assert!(parse_page_ranges("1,,2").is_err());
  }

  #[test]
  fn a_page_flag_takes_what_css_takes() {
    assert!(check_page_declaration("size", "letter landscape").is_ok());
    assert!(check_page_declaration("margin", "1in 0.5in").is_ok());
    assert!(check_page_declaration("size", "bogus").is_err());
    assert!(check_page_declaration("margin", "1in !important").is_err());
    assert!(
      check_page_declaration("margin", "1cm } body { display: none } @page { margin: 1cm").is_err()
    );
  }
}
