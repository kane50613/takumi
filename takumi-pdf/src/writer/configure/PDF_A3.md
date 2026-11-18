# Description

PDF/A-3 requires a PDF version <= 1.7 and defines three conformance levels,
in the following order from less strict to more strict:

- Level B
- Level U
- Level A

It is exactly the same as PDF/A-2, except for clause 6.8.

See `README.md` for the meaning of each subclause.

## Level B

## 6.8 Embedded files

- krilla allows embedded files in this export mode. 🟢
  // TODO: Decide whether UF should be written for < PDF 1.7.
- krilla writes `F` and `AFRelationship` for embedded files, and writes `UF` for PDF 1.7
  exports. 🟠
- krilla requires modification date, description and MIME subtype for embedded files in this
  export mode. 🟢
- The fact that embedded file relationships should correctly describe the relationship to
  the document is documented. 🟣

## Level U

-

# Level A

-
