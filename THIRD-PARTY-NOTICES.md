# Third-party notices

The native and WebAssembly packages contain code that other projects license under the terms below.

## Independent JPEG Group

`@takumi-rs/core` and `@takumi-rs/wasm` encode JPEG with a forward DCT derived from the Independent JPEG Group's libjpeg.

This software is based in part on the work of the Independent JPEG Group.

## Chromium and Skia

`@takumi-rs/core`, `@takumi-rs/wasm`, `takumi-pdf`, and `takumi-paint` contain parts of `takumi-core` that follow algorithms from Chromium's Blink renderer and from Skia. [`takumi-core/LICENSE-CHROMIUM`](takumi-core/LICENSE-CHROMIUM) names each part and the notice its source carries.

## emoji-segmenter

`@takumi-rs/core`, `@takumi-rs/wasm`, `takumi-pdf`, and `takumi-paint` split text into emoji and text runs with a rewrite of the grammar from [emoji-segmenter](https://github.com/google/emoji-segmenter), under the Apache License 2.0. [`takumi-core/LICENSE-EMOJI-SEGMENTER`](takumi-core/LICENSE-EMOJI-SEGMENTER) names the rewritten file and carries the license.

## ICU

`@takumi-rs/core`, `@takumi-rs/wasm`, `takumi-pdf`, and `takumi-paint` give removed bidi controls and trailing white space their levels as ICU does, under the Unicode License v3. [`takumi-core/LICENSE-ICU`](takumi-core/LICENSE-ICU) names the part and carries the license.

## krilla and subsetter

`takumi-pdf` writes PDF with a vendored copy of [krilla](https://github.com/LaurenzV/krilla) 0.8.2 and subsets fonts with a vendored copy of [subsetter](https://github.com/typst/subsetter) 0.2.6, both under the MIT License or the Apache License 2.0. [`takumi-pdf/LICENSE-KRILLA`](takumi-pdf/LICENSE-KRILLA) and [`takumi-pdf/LICENSE-SUBSETTER`](takumi-pdf/LICENSE-SUBSETTER) carry their notices.

## libwebp

`@takumi-rs/core` encodes WebP with libwebp.

```text
Copyright (c) 2010, Google Inc. All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

  * Redistributions of source code must retain the above copyright
    notice, this list of conditions and the following disclaimer.

  * Redistributions in binary form must reproduce the above copyright
    notice, this list of conditions and the following disclaimer in
    the documentation and/or other materials provided with the
    distribution.

  * Neither the name of Google nor the names of its contributors may
    be used to endorse or promote products derived from this software
    without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
"AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

```
