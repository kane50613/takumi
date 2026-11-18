# Description

PDF/A-2 requires PDF version <= 1.7 and defines three conformance levels,
in the following order from less strict to more strict:

- Level B
- Level U
- Level A

Level U is a subset of level A, and level B is a subset of level U.

See `README.md` for the meaning of each color.

## Level B

## 6.1 File structure

6.1.2: `pdf-writer` always write the file header as described in the spec. Even if the
user enables `ascii_compatible`, we still write a binary header marker. 🟢

6.1.3:

- We always set the file ID. 🟢
- We do not support encryption. 🔵

6.1.4: `pdf-writer` always write the xref section as described in the spec. 🟢

6.1.5: -

6.1.6: `pdf-writer` always writes hex strings with an even number of characters. 🟢

6.1.7:

- `pdf-writer` always write streams as described in the spec. 🟢
- krilla does never write streams referencing external files. 🔵
- krilla does never use `LZWDecode` or `Crypt`. 🔵
- krilla never writes F, FFIlter or FDecodeParams in streams. 🔵

6.1.8: krilla only ever writes UTF-8 strings as names. 🟢

6.1.9: `pdf-writer` always writes indirect objects as described in the spec. 🟢

6.1.10: krilla does never use inline image dictionaries. 🔵

6.1.11: -

6.1.12: krilla doesn't support permissions. 🔵

6.1.13:

- `pdf-writer` uses i32 for integers. 🟢
- `pdf-writer` uses f32 for real numbers. 🟢
- krilla always uses the `new_str` and `new_text_str` methods of the SerializeContext to create them,
  which returns a validation error in case it's too long. 🔵
- krilla trims the names of fonts, and all other names cannot be longer than 127. 🔵
- krilla fails export if more than 8388607 indirect objects exist. 🟢
- krilla fails export if a higher nesting-level than 28 exists. 🟢
- krilla does not use the DeviceN color space. 🟢
- krilla only uses u16 for CIDs. 🟢

## 6.2 Graphics

6.2.2:

- krilla doesn't use non-standard operators. 🟢
- krilla doesn't use the `ri` or `i` operator. 🟢
- krilla ensure that content stream has their own associated resource dictionary. 🟢

6.2.3: krilla always write an `sRGB` output intent for PDF/A. 🟢

6.2.4.1: krilla overrides the `no_device_cs` property if PDF/A is selected, and
in case CMYK is used but no profile was provided, export fails.

6.2.4.2:

- sRGB/sGrey ICC profiles conform to ICC v4 specification. 🟢
- krilla does not support overprinting. 🔵

6.2.4.3:

- krilla always uses sRGB for RGB, and in addition also embeds an sRGB output intent. 🟢
- krilla always uses sGrey (except for encoding the alpha channel in images, where DeviceGray
  is required), and in addition always embeds an output intent. 🟢
- krilla always uses an CMYK ICC profile, and always sets CMYK as the output intent. 🟢
  It fails export if no CMYK ICC profile was provided. 🟢

6.2.4.4:

- krilla ensures the Alternate space in Separation color spaces obeys the restrictions
  in the applicable clauses 🟢
- krilla does not support DeviceN color spaces. 🔵
- krilla fails export if a Separation colorant is associated with multiple different
  fallback color spaces 🟢
- krilla manages the `tintTransform` function and will always write the same function for
  the same color space 🟢

6.2.4.5: Fulfilled because patterns are treated the same as all other elements in krilla. 🟢

6.2.5: krilla does not use the transfer functions, halftones, TR/HTP/RI/FL keys. 🔵

6.2.6: krilla does never define a rendering intent. 🔵

6.2.7: krilla is not a reader. 🔵

6.2.8.1:

- krilla does not use the `Alternates`/`Intent` keys for images. 🔵
- krilla does check whether the `Interpolate` key is used. 🟢

6.2.8.2: krilla does not support thumbnails. 🔵

6.2.8.3: krilla doesn't support JPEG2000 images. 🔵

6.2.9.1: krilla does not use the `OPI`/`Subtype2`/`PS` keys for XObjects. 🔵

6.2.9.2: krilla does not use reference XObjects. 🔵

6.2.9.3: The spec only talks about PostScript XObjects, which we don't really use.
We only use PostScript functions. In any case, to be on the safe side, krilla fails exports
when a PostScript function is used. 🟢

6.2.10: krilla always includes an OutputIntent for PDF/A, so the /G attribute is not
always required. 🟢

6.2.11.1: krilla fails export in PDF/A when the .notdef glyph is referenced. 🟢

6.2.11.2: krilla has made sure that the spec is followed in this regard. 🟢

6.2.11.3.1: krilla always uses `Identity-H` as encoding. 🟢

6.2.11.3.2: krilla always writes the `CIDToGidMap` entry. 🟢

6.2.11.3.3: krilla always writes the `WMode` entry for cmaps and never references any other ones. 🟢

6.2.11.4.1:

- krilla embeds all fonts that are used. 🟢
- krilla checks the OpenType fsType field to ensure that fonts are legally embeddable. 🟢

6.2.11.4.2:

- krilla never writes the `CharSet` attribute. 🔵
- krilla always includes all CIDs in `CIDSet`. 🔵

6.2.11.5: krilla copies the font metrics directly from the font. 🟢

6.2.11.6:

- krilla embeds all fonts as symbolic. 🟢
- krilla does not write TrueType fonts directly. 🔵
- krilla only writes CIDFonts instead of TrueType fonts directly, so cmap is not needed. 🟢

6.2.11.8: krilla fails export when the .notdef glyph is referenced. 🟢

## 6.3 Annotations

6.3.1: krilla does not support any non-standard annotation types, nor `3D`, `Sound`, `Screen` or `Movie`. 🔵

6.3.2:

- krilla always sets the `F` flag for annotations. 🟢
- krilla does not support text annotations. 🔵

6.3.3: krilla only supports the Link subtype for annotations, which doesn't require an appearence stream. 🔵

6.3.4: -

## 6.4 Interactive forms

6.4.1: krilla does not support interactive forms. 🔵

6.4.2: krilla does not support interactive forms. 🔵

6.4.3: krilla does not support digital signatures. 🔵

## 6.5 Action

6.5.1:

- krilla does not support the `Launch`, `Sound`, `Movie`, `ResetForm`, `ImportData`, `Hide`, `SetOCGState`, `Rendition`, `Trans`,
  `GoTo3DView` and `JavaScript` actions. 🔵
- krilla does not use the `set-state` and `no-op` actions. 🔵
- krilla does not support named actions. 🔵

6.5.2:

- krilla does not support widget annotations. 🔵
- krilla does not write an AA entry in the document catalog. 🟢
- krilla does not write an AA entry for pages. 🟢

6.5.3: -

## 6.6 Metadata

6.6.1: -

6.6.2.1:

- krilla overrides the `xmp_metadata` attribute for PDF/A exports so that it's always contained. 🟢
- `xmp-writer` always creates conforming XMP streams. 🟢
- We never include the `bytes` or `encoding` attributes. 🟢

6.6.2.2: -

6.6.2.3.1: krilla doesn't use any non-standard properties. 🟢

6.6.2.3.2: krilla writes the extension schemas. 🟢

6.6.2.3.3: krilla writes the extension schemas. 🟢

6.6.3: krilla ensures that XMP metadata and document info dictionary are consistent. 🟢

6.6.4: krilla writes pdfaid:conformance and pdfaid:part as specified. 🟢

6.6.5:

- krilla writes a document and instance ID. 🟢
- krilla writes a minimal `xmpMM:History` entry when a creation date is provided. 🟢

6.6.6: krilla writes a minimal `xmpMM:History` entry when a creation date is
provided. 🟢

# 6.8 Embedding files

krilla prohibits embedding files in this export mode. 🟢

# 6.9 Optional content

krilla does not support optional content. 🔵

# 6.10 Use of alternate presentations and transitions

- krilla never writes `AlternatePresentations` in the name dictionary. 🔵
- krilla never writes `PresSteps` into page dictionaries. 🔵

# 6.11 Document requirements

krilla never writes the `Requirements` key in the document dictionary. 🔵

## Level U

6.2.11.7.1: -

6.2.11.7.2:

- krilla always includes a `ToUnicode` mapping. 🟢
- For levels U and A, krilla checks that all glyphs have a codepoint mapping that
  does not contain 0x0, 0xFEFF or 0xFFFE. 🟢

# Level A

6.2.11.7.3: krilla forbids all codepoints in the unicode private area for this export mode. 🟢

# 6.7 Logical Structure

6.7.1: -

6.7.2.1: General conformance to tagged PDF cannot be checked, but is documented. 🟣

6.7.2.2: krilla always writes the mark info dictionary for this export mode. 🟢

6.7.3.1: The need to specify artifacts is documented. 🟣

6.7.3.2: The need to specify word boundaries is documented. 🟣

6.7.3.3:

- The presence of a structure tree is enforced. 🟢
- The need to specify a granular structure hierarchy is documented. 🟣

6.7.3.4: krilla maps all non-standard structure types. 🟢

6.7.4:

- krilla requires the user to set the document language in that export mode. 🟢
- krilla allows users to specify the language on spans. 🟢
- The need for correctness of language tags is documented. 🟣

6.7.5: The fact that an alternate text should be provided to figures and formulas is checked. 🟢

6.7.6: krilla ensures that annotations have an alternate text. 🟢

6.7.7: The requirement to specify replacement text is documented. 🟣

6.7.8: The requirement to specify the expansions of abbreviations is documented. 🟣
NOTE: When inserting expansions of abbreviations, you must set them on the
leaf `SpanTag` struct passed to `Surface::start_tagged` method instead of the
`Tag` struct in the structure tree
